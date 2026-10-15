//! Qualified read-only metadata capture before mutating DIRECT owner acquisition.
use super::{
    native_objects as objects,
    native_request::{NativeRootRequest, NativeWork},
};
use eliot_searchd::native_file::{self, Observation};
use search_runtime_owner::OwnerError;
use search_source_admission::{self as admission, policy_v2::CheckpointFailureV2};
use search_source_provenance::{
    OriginalInputsLayoutV2, RetainedPolicyV2, MAX_RETAINED_POLICY_BYTES,
};
use std::{
    fs::File,
    path::{Component, Path, PathBuf, Prefix},
    sync::Arc,
};

pub(super) struct CapturedPolicyV2 {
    pub(super) retained: Arc<RetainedPolicyV2>,
    pub(super) reference: Arc<[u8]>,
}

fn local_path(path: &Path) -> Result<(), OwnerError> {
    if !cfg!(all(windows, target_arch = "x86_64"))
        || !path.is_absolute()
        || path.as_os_str().len() > 2048
    {
        return Err(OwnerError::DataRootInvalid);
    }
    if path.to_str().is_none_or(|value| {
        value
            .split(['\\', '/'])
            .any(|part| matches!(part, "." | ".."))
    }) {
        return Err(OwnerError::DataRootInvalid);
    }
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Prefix(prefix))
        if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        || components.next() != Some(Component::RootDir)
    {
        return Err(OwnerError::DataRootInvalid);
    }
    for component in components {
        let Component::Normal(name) = component else {
            return Err(OwnerError::DataRootInvalid);
        };
        let name = name.to_str().ok_or(OwnerError::DataRootInvalid)?;
        if name.contains(':') || name.ends_with(['.', ' ']) {
            return Err(OwnerError::DataRootInvalid);
        }
    }
    Ok(())
}

fn observe(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    file: &File,
) -> Result<Observation, OwnerError> {
    work.call(request, || {
        native_file::observe(file).map_err(|_| OwnerError::OwnerGuardMismatch)
    })
}

fn canonical_path(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
) -> Result<PathBuf, OwnerError> {
    let result = work.call(request, || {
        std::fs::canonicalize(path).map_err(|_| OwnerError::DataRootInvalid)
    })?;
    local_path(&result)?;
    Ok(result)
}

pub(super) fn capture(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    path: &Path,
) -> Result<CapturedPolicyV2, OwnerError> {
    request.check()?;
    local_path(path)?;
    work.read(request, 16 * 1024)?;
    let canonical = canonical_path(request, work, path)?;
    let parent = canonical.parent().ok_or(OwnerError::DataRootInvalid)?;
    let mut pins = Vec::with_capacity(8);
    let mut ancestor = PathBuf::new();
    for component in parent.components() {
        ancestor.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        if pins.len() == 8 {
            return Err(OwnerError::ContractExhausted);
        }
        let file = objects::open_initialization(request, work, &ancestor, true)?;
        let observed = observe(request, work, &file)?;
        pins.push((ancestor.clone(), file, observed));
    }
    if pins.is_empty() {
        return Err(OwnerError::DataRootInvalid);
    }
    // FILE_SHARE_READ excludes both existing and later ordinary write/delete
    // access. Keep this FD and every ancestor through restore and final fences.
    let mut file = objects::open_initialization(request, work, &canonical, false)?;
    let before = observe(request, work, &file)?;
    let bytes = objects::read_bytes(request, work, &mut file, MAX_RETAINED_POLICY_BYTES)?;
    work.read(request, bytes.len().saturating_mul(16))?;
    let platform = admission::SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = admission::AcceptedCapabilities {
        profiles: vec![admission::BASELINE_PROFILE.into()],
    };
    let mut first = None;
    let mut probe = || {
        request.check().map_err(|error| {
            if first.is_none() {
                first = Some(error);
            }
            if error == OwnerError::OwnerCancelledBeforeMutation {
                CheckpointFailureV2::Cancelled
            } else {
                CheckpointFailureV2::DeadlineExceeded
            }
        })
    };
    let restored = RetainedPolicyV2::restore(&bytes, &platform, &capabilities, &mut probe)
        .and_then(|retained| {
            OriginalInputsLayoutV2::policy_reference(&retained, &mut probe).map(|reference| {
                CapturedPolicyV2 {
                    retained: Arc::new(retained),
                    reference: reference.into(),
                }
            })
        });
    if let Some(error) = first {
        return Err(error);
    }
    request.check()?;
    let restored = restored.map_err(|_| OwnerError::OwnerOperationConflict)?;
    for locator in [path, canonical.as_path()] {
        let current = objects::open_initialization(request, work, locator, false)?;
        if observe(request, work, &current)? != before {
            return Err(OwnerError::OwnerGuardMismatch);
        }
    }
    if observe(request, work, &file)? != before || canonical_path(request, work, path)? != canonical
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    for (locator, file, before) in &pins {
        let current = objects::open_initialization(request, work, locator, true)?;
        for after in [
            observe(request, work, file)?,
            observe(request, work, &current)?,
        ] {
            if (
                after.volume_serial,
                after.file_index,
                after.creation_time,
                after.attributes,
            ) != (
                before.volume_serial,
                before.file_index,
                before.creation_time,
                before.attributes,
            ) {
                return Err(OwnerError::OwnerGuardMismatch);
            }
        }
    }
    request.check()?;
    Ok(restored)
}
