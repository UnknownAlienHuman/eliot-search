//! Consumed source policy/currentness preparation, before any native mutation.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use search_source_registry::encode_legacy_source_root_catalog;

use crate::owner_composition::NativeRegistrationOperation;
use crate::service_output::json_string;

use super::SourceRootInspection;
use super::error::SourceRootError;
use super::inspection::{BoundedOutput, SourceRootProbes, observe};
use super::path::{
    ensure_no_overlap, ensure_outside_data_root, is_reparse, path_text, validate_persisted_path,
};
use super::registry::map_codec_error;
use super::spec::MAX_SOURCE_ROOTS;

#[derive(Clone, Copy)]
pub enum SourceRootAction {
    Register,
    Unregister,
}

pub struct PreparedSourceRootChange {
    replacement: Option<Vec<u8>>,
    planned_output: Vec<u8>,
}

impl PreparedSourceRootChange {
    pub(crate) fn replacement(&self) -> Option<&[u8]> {
        self.replacement.as_deref()
    }

    pub(crate) fn output(&self) -> &[u8] {
        &self.planned_output
    }

    pub(crate) fn prepare(
        operation: &mut NativeRegistrationOperation<'_>,
        action: SourceRootAction,
        requested: &Path,
    ) -> Result<Self, String> {
        let bytes = operation.registration_bytes()?;
        let snapshot = SourceRootInspection::read_using(bytes.as_deref(), operation)?;
        snapshot.prepare_using(action, requested, operation)
    }
}

impl SourceRootInspection {
    fn prepare_using(
        mut self,
        action: SourceRootAction,
        requested: &Path,
        probes: &mut impl SourceRootProbes,
    ) -> Result<PreparedSourceRootChange, String> {
        let (changed, event_path) = match action {
            SourceRootAction::Register => {
                let canonical = canonical_directory(probes, requested)?;
                ensure_outside_data_root(&canonical, probes.root())
                    .map_err(|error| error.code().to_owned())?;
                if let Some(index) = self.paths.iter().position(|path| path == &canonical) {
                    let state = observe(probes, &canonical)?;
                    self.currentness
                        .observe(index, state)
                        .map_err(|error| error.code().to_owned())?;
                    (false, canonical)
                } else {
                    if self.paths.len() >= MAX_SOURCE_ROOTS {
                        return Err(SourceRootError::RootLimitExceeded.code().to_owned());
                    }
                    ensure_no_overlap(self.paths.iter(), &canonical)
                        .map_err(|error| error.code().to_owned())?;
                    let index = self.paths.partition_point(|path| path < &canonical);
                    self.paths.insert(index, canonical.clone());
                    self.currentness
                        .insert(index, super::SourceRootState::Available)
                        .map_err(|error| error.code().to_owned())?;
                    (true, canonical)
                }
            }
            SourceRootAction::Unregister => {
                let index =
                    if let Some(index) = self.paths.iter().position(|path| path == requested) {
                        index
                    } else {
                        let canonical = canonical_directory(probes, requested)?;
                        self.paths
                            .iter()
                            .position(|path| path == &canonical)
                            .ok_or_else(|| SourceRootError::RootNotFound.code().to_owned())?
                    };
                let removed = self.paths.remove(index);
                self.currentness
                    .remove(index)
                    .map_err(|error| error.code().to_owned())?;
                (true, removed)
            }
        };
        probes.checkpoint()?;
        let replacement = if changed {
            let text = self
                .paths
                .iter()
                .map(|path| path_text(path))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.code().to_owned())?;
            Some(
                encode_legacy_source_root_catalog(text)
                    .map_err(map_codec_error)
                    .map_err(|error| error.code().to_owned())?,
            )
        } else {
            None
        };
        let path = path_text(&event_path).map_err(|error| error.code().to_owned())?;
        let mut output = BoundedOutput(Vec::new());
        let event = match action {
            SourceRootAction::Register => writeln!(
                output,
                "{{\"event\":\"source_root_registered\",\"path\":{},\"persisted\":true,\"access_granted\":false}}",
                json_string(path)
            ),
            SourceRootAction::Unregister => writeln!(
                output,
                "{{\"event\":\"source_root_unregistered\",\"path\":{},\"persisted\":true,\"retained_revisions_revoked\":false}}",
                json_string(path)
            ),
        };
        event
            .and_then(|()| self.write_output(&mut output))
            .map_err(|_| "SOURCE_ROOT_OUTPUT_TOO_LARGE".to_owned())?;
        probes.checkpoint()?;
        Ok(PreparedSourceRootChange {
            replacement,
            planned_output: output.0,
        })
    }
}

fn canonical_directory(
    probes: &mut impl SourceRootProbes,
    requested: &Path,
) -> Result<PathBuf, String> {
    reject_symlink(probes, requested)?;
    let canonical = probes
        .source_root_canonicalize(requested)?
        .map_err(|error| SourceRootError::RootIo(error).code().to_owned())?;
    reject_symlink(probes, &canonical)?;
    let metadata = probes
        .source_root_metadata(&canonical)?
        .map_err(|error| SourceRootError::RootIo(error).code().to_owned())?;
    if !metadata.is_dir() {
        return Err(SourceRootError::RootNotDirectory.code().to_owned());
    }
    validate_persisted_path(&canonical).map_err(|error| error.code().to_owned())?;
    Ok(canonical)
}

fn reject_symlink(probes: &mut impl SourceRootProbes, path: &Path) -> Result<(), String> {
    match probes.source_root_metadata(path)? {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
            Err(SourceRootError::SymlinkDenied.code().to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error).code().to_owned()),
    }
}
