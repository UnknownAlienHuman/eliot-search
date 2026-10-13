//! One immutable registration observation; it cannot acknowledge synchronization.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use search_source_registry::SourceRootCurrentness;

use crate::owner_composition::{
    NativeReadOperation, NativeRegistrationOperation, NativeSyncPreflightOperation,
    SourceDirectoryMetadata as Metadata,
};
use crate::service_output::json_string;

use super::SourceRootState;
use super::path::{canonicalize_configured_set, ensure_outside_data_root, is_reparse, path_text};
use super::registry::decode_configured_paths;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Complete existing root policy/currentness preflight; never acknowledges sync.
pub fn preflight_sync(operation: &mut NativeSyncPreflightOperation<'_>) -> Result<(), String> {
    let bytes = operation
        .registration_bytes()
        .map_err(|error| error.code().to_owned())?;
    let inspection = SourceRootInspection::read_using(bytes.as_deref(), operation)?;
    let truth = inspection.currentness.current_workspace_truth();
    if truth.configured == 0
        || !truth.source_current
        || truth.unavailable != 0
        || truth.gap_count != 0
    {
        return Err("SOURCE_ROOT_SYNC_UNAVAILABLE".to_owned());
    }
    operation.verify().map_err(|error| error.code().to_owned())
}

pub struct SourceRootInspection {
    pub(super) paths: Vec<PathBuf>,
    pub(super) currentness: SourceRootCurrentness,
}

impl SourceRootInspection {
    pub(crate) fn read(operation: &mut NativeReadOperation<'_>) -> Result<Self, String> {
        let bytes = operation.registration_bytes()?;
        Self::read_using(bytes.as_deref(), operation)
    }

    pub(super) fn read_using(
        bytes: Option<&[u8]>,
        operation: &mut impl SourceRootProbes,
    ) -> Result<Self, String> {
        operation.checkpoint()?;
        let mut paths = bytes
            .map(decode_configured_paths)
            .transpose()
            .map_err(|error| error.code().to_owned())?
            .unwrap_or_default();
        canonicalize_configured_set(&mut paths).map_err(|error| error.code().to_owned())?;
        for path in &paths {
            ensure_outside_data_root(path, operation.root())
                .map_err(|error| error.code().to_owned())?;
        }
        let mut observed = Vec::with_capacity(paths.len());
        for path in &paths {
            observed.push(observe(operation, path)?);
        }
        let mut currentness =
            SourceRootCurrentness::new(paths.len()).map_err(|error| error.code().to_owned())?;
        currentness.reconcile(&observed);
        Ok(Self { paths, currentness })
    }

    pub(crate) fn output(&self) -> Result<Vec<u8>, String> {
        let mut output = BoundedOutput(Vec::new());
        self.write_output(&mut output)
            .map_err(|_| "SOURCE_ROOT_OUTPUT_TOO_LARGE".to_owned())?;
        Ok(output.0)
    }

    pub(super) fn write_output(&self, writer: &mut impl Write) -> io::Result<()> {
        for (position, path) in self.paths.iter().enumerate() {
            let path =
                path_text(path).map_err(|_| io::Error::other("SOURCE_ROOT_PATH_NOT_UTF8"))?;
            let state = self
                .currentness
                .state(position)
                .ok_or_else(|| io::Error::other("SOURCE_ROOT_CATALOG_CORRUPT"))?;
            writeln!(
                writer,
                "{{\"event\":\"source_root\",\"position\":{position},\"path\":{},\"state\":\"{}\"}}",
                json_string(path),
                state.code()
            )?;
        }
        for gap in self.currentness.observation_gaps() {
            writeln!(
                writer,
                "{{\"event\":\"source_gap\",\"position\":{},\"reason\":\"{}\",\"state\":\"{}\"}}",
                gap.position,
                gap.reason.code(),
                gap.state.code()
            )?;
        }
        let truth = self.currentness.current_workspace_truth();
        let cursor = self.currentness.reconciliation_cursor();
        let proven = crate::provider_composition::evaluate_current_workspace_proven(
            truth.configured,
            truth.gap_count,
            truth.workspace_current,
            false,
        );
        let reason = crate::provider_composition::current_workspace_proven_reason(
            truth.configured,
            truth.gap_count,
            truth.workspace_current,
            false,
        );
        writeln!(
            writer,
            concat!(
                "{{\"event\":\"source_roots_complete\",\"configured\":{},",
                "\"available\":{},\"unavailable\":{},\"gaps\":{},",
                "\"reconciliation_generation\":{},\"watcher_pending\":{},",
                "\"watcher_overflowed\":{},\"proven_reason\":\"{}\",",
                "\"current_workspace_proven\":{}}}"
            ),
            truth.configured,
            truth.available,
            truth.unavailable,
            truth.gap_count,
            truth.reconciliation_generation,
            cursor.pending_hints,
            cursor.overflowed,
            reason,
            proven
        )
    }
}

pub(super) fn observe(
    operation: &mut impl SourceRootProbes,
    path: &Path,
) -> Result<SourceRootState, String> {
    let metadata = match operation.source_root_metadata(path)? {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SourceRootState::Missing);
        }
        Err(_) => return Ok(SourceRootState::Unverifiable),
    };
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Ok(SourceRootState::Unsafe);
    }
    if !metadata.is_dir() {
        return Ok(SourceRootState::NotDirectory);
    }
    Ok(match operation.source_root_canonicalize(path)? {
        Ok(canonical) if canonical == path => SourceRootState::Available,
        Ok(_) => SourceRootState::Unsafe,
        Err(_) => SourceRootState::Unverifiable,
    })
}

pub(super) struct BoundedOutput(pub(super) Vec<u8>);

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("SOURCE_ROOT_OUTPUT_TOO_LARGE"));
        }
        self.0
            .try_reserve_exact(bytes.len())
            .map_err(|_| io::Error::other("SOURCE_ROOT_OUTPUT_TOO_LARGE"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Two closed native callers share source policy and currentness, not authority.
pub(super) trait SourceRootProbes {
    fn root(&self) -> &Path;
    fn checkpoint(&mut self) -> Result<(), String>;
    fn source_root_metadata(&mut self, path: &Path) -> Result<io::Result<Metadata>, String>;
    fn source_root_canonicalize(&mut self, path: &Path) -> Result<io::Result<PathBuf>, String>;
}

impl SourceRootProbes for NativeReadOperation<'_> {
    fn root(&self) -> &Path {
        Self::root(self)
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        Self::checkpoint(self).map_err(|error| error.code().to_owned())
    }
    fn source_root_metadata(&mut self, path: &Path) -> Result<io::Result<Metadata>, String> {
        Self::source_root_metadata(self, path).map_err(|error| error.code().to_owned())
    }
    fn source_root_canonicalize(&mut self, path: &Path) -> Result<io::Result<PathBuf>, String> {
        Self::source_root_canonicalize(self, path).map_err(|error| error.code().to_owned())
    }
}

impl SourceRootProbes for NativeRegistrationOperation<'_> {
    fn root(&self) -> &Path {
        Self::root(self)
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        Self::checkpoint(self).map_err(|error| error.code().to_owned())
    }
    fn source_root_metadata(&mut self, path: &Path) -> Result<io::Result<Metadata>, String> {
        Self::source_root_metadata(self, path).map_err(|error| error.code().to_owned())
    }
    fn source_root_canonicalize(&mut self, path: &Path) -> Result<io::Result<PathBuf>, String> {
        Self::source_root_canonicalize(self, path).map_err(|error| error.code().to_owned())
    }
}

impl SourceRootProbes for NativeSyncPreflightOperation<'_> {
    fn root(&self) -> &Path {
        Self::root(self)
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        Self::checkpoint(self).map_err(|error| error.code().to_owned())
    }
    fn source_root_metadata(&mut self, path: &Path) -> Result<io::Result<Metadata>, String> {
        Self::source_root_metadata(self, path).map_err(|error| error.code().to_owned())
    }
    fn source_root_canonicalize(&mut self, path: &Path) -> Result<io::Result<PathBuf>, String> {
        Self::source_root_canonicalize(self, path).map_err(|error| error.code().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_escaped_observations_keep_gaps_counts_and_unproven_truth() {
        let mut currentness = SourceRootCurrentness::new(3).unwrap();
        currentness.reconcile(&[
            SourceRootState::Available,
            SourceRootState::Missing,
            SourceRootState::NotDirectory,
        ]);
        let snapshot = SourceRootInspection {
            paths: vec![
                PathBuf::from("C:\\one\""),
                PathBuf::from("C:\\two"),
                PathBuf::from("C:\\three"),
            ],
            currentness,
        };
        let output = String::from_utf8(snapshot.output().unwrap()).unwrap();
        let rows: Vec<_> = output.lines().collect();
        assert_eq!(rows.len(), 6);
        assert_eq!(
            rows[0],
            "{\"event\":\"source_root\",\"position\":0,\"path\":\"C:\\\\one\\\"\",\"state\":\"available\"}"
        );
        assert!(rows[3].contains("\"position\":1"));
        assert!(rows[4].contains("\"position\":2"));
        assert!(rows[5].contains("\"configured\":3,\"available\":1,\"unavailable\":2,\"gaps\":2"));
        assert!(rows[5].ends_with("\"current_workspace_proven\":false}"));
        assert!(
            !snapshot
                .currentness
                .current_workspace_truth()
                .workspace_current
        );
    }

    #[test]
    fn output_capacity_is_checked_before_copy_and_refuses_whole_result() {
        let mut output = BoundedOutput(vec![0; MAX_OUTPUT_BYTES - 1]);
        assert!(output.write(&[1, 2]).is_err());
        assert_eq!(output.0.len(), MAX_OUTPUT_BYTES - 1);
        assert_eq!(output.write(&[1]).unwrap(), 1);
        assert!(output.write(&[2]).is_err());
    }
}
