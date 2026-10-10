//! Exact original catalog-mutation inputs at the existing quarantine locators.
//!
//! This immutable record is evidence, never a read/write or recovery capability.
//! Its token is minted only under the same live owner and original request. Drop
//! and every error retain the staging/final evidence; no create is replayed.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalText, CanonicalValue, parse_canonical_cbor,
    to_canonical_cbor,
};
use search_runtime_owner::OwnerError;

use super::lifecycle::LiveOwner;
use super::operation::DataRootRequest;
use super::read_existing::{read_existing_bytes, verify_existing_locator};
use super::record::DurableOwnerRecord;
use super::spec::{DrainReasonText, LifecycleState};
use crate::catalog_quarantine::{QUARANTINE_ARM_FAILED, QUARANTINE_CLEAR_FAILED, QUARANTINE_ERROR};

pub(super) const FORMAT: &str = "eliot-search/catalog-mutation-intent/v2";
const MARKER: &str = "catalog-quarantine.marker";
const STAGING: &str = "catalog-quarantine.tmp";
pub(super) const MAX_INTENT_BYTES: usize = 512 * 1024;

/// Retained exact publication token. It cannot be cloned or decoded into authority.
pub struct CatalogMutationIntent {
    root: PathBuf,
    file: Option<File>,
    bytes: Vec<u8>,
    request: DataRootRequest,
    owner_record: DurableOwnerRecord,
    value: CanonicalValue,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::development::DataRootGuard;
    use crate::owner_composition::{InitializationRequest, initialize_new_request};
    use crate::root_draining_266::{Scratch, snapshot};
    use std::path::Path;

    const INITIALIZATION_ID: &str = "26600000000000000000000000000008";

    fn initialized() -> Scratch {
        let scratch = Scratch::new();
        let request = DataRootRequest::from_cli(&[
            "--initialize-data-root".into(),
            scratch.0.as_os_str().to_owned(),
            INITIALIZATION_ID.into(),
        ])
        .unwrap();
        let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
        initialize_new_request(&scratch.0, &initialization, &request).unwrap();
        scratch
    }

    fn mutation(root: &Path) -> DataRootRequest {
        DataRootRequest::from_cli(&[
            "--gc-root".into(),
            root.as_os_str().to_owned(),
            "--apply".into(),
        ])
        .unwrap()
    }

    // The live primary/sealed exclusions byte-lock their files on Windows.
    // Capture the control tree while held, then the full tree after release.
    fn control_snapshot(root: &Path) -> crate::root_draining_266::TreeSnapshot {
        snapshot(&root.join("control"))
    }

    #[test]
    fn owned_intent_retains_exact_input_until_actual_clean_release() {
        use std::os::windows::ffi::OsStrExt;

        let scratch = initialized();
        let root = &scratch.0;
        let request = mutation(root);
        let mut guard = DataRootGuard::open_existing_request(root, &request).unwrap();
        let intent = guard.arm_catalog_intent(&request).unwrap();
        assert_eq!(format!("{intent:?}"), "CatalogMutationIntent(<opaque>)");
        let marker = fs::read(root.join("control").join(MARKER)).unwrap();
        assert_eq!(
            fs::read(root.join("control").join(STAGING)).unwrap(),
            marker
        );
        let CanonicalValue::Array(fields) = parse_canonical_cbor(&marker).unwrap() else {
            panic!("intent is not the closed array");
        };
        assert_eq!(fields.len(), 4);
        assert_eq!(fields.as_slice()[1], CanonicalValue::U64(2));
        let CanonicalValue::Bytes(owner_bytes) = &fields.as_slice()[2] else {
            panic!("owner record bytes absent");
        };
        let record = DurableOwnerRecord::decode(owner_bytes.as_slice()).unwrap();
        let current = super::super::slots::newest_valid(root).unwrap().1.unwrap();
        assert_eq!(record, *current);
        assert_eq!(record.epoch, 2);
        assert_eq!(record.generation, 4);
        assert_eq!(record.lifecycle, LifecycleState::Active);
        let CanonicalValue::Array(input) = &fields.as_slice()[3] else {
            panic!("original input absent");
        };
        assert_eq!(input.len(), 4);
        assert_eq!(
            input.as_slice()[2],
            CanonicalValue::Bytes(
                BoundedBytes::new(request.operation().request_digest().as_bytes().to_vec())
                    .unwrap()
            )
        );
        let CanonicalValue::Array(arguments) = &input.as_slice()[3] else {
            panic!("original native arguments absent");
        };
        assert_eq!(arguments.len(), 3);
        let native_root = root
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            arguments.as_slice()[1],
            CanonicalValue::Bytes(BoundedBytes::new(native_root).unwrap())
        );
        guard
            .begin_drain(search_runtime_owner::DrainReason::Shutdown)
            .unwrap();
        assert_eq!(fs::read(root.join("control").join(MARKER)).unwrap(), marker);
        let receipt = guard.release_catalog_cleanly(intent, &request).unwrap();
        assert_eq!(receipt.epoch.get(), 2);
        assert_eq!(receipt.generation, 6);
        assert!(!root.join("control").join(MARKER).exists());
        assert!(!root.join("control").join(STAGING).exists());
        let inspection =
            DataRootRequest::from_cli(&["--health-data-root".into(), root.as_os_str().to_owned()])
                .unwrap();
        DataRootGuard::with_inspection_request(root, &inspection, |_| Ok(())).unwrap();
    }

    #[test]
    fn foreign_context_repeat_arm_and_drop_preserve_exact_evidence() {
        let scratch = initialized();
        let root = &scratch.0;
        let request = mutation(root);
        let foreign = mutation(root);
        let guard = DataRootGuard::open_existing_request(root, &request).unwrap();
        let before = control_snapshot(root);
        assert_eq!(
            guard.arm_catalog_intent(&foreign).err().as_deref(),
            Some("OWNER_OPERATION_CONFLICT")
        );
        assert_eq!(control_snapshot(root), before);
        let intent = guard.arm_catalog_intent(&request).unwrap();
        let armed = control_snapshot(root);
        assert_eq!(
            guard.arm_catalog_intent(&request).err().as_deref(),
            Some(QUARANTINE_ERROR)
        );
        assert_eq!(control_snapshot(root), armed);
        drop(intent);
        drop(guard);
        assert_eq!(control_snapshot(root), armed);
        let retained = snapshot(root);
        let result: Result<(), String> =
            DataRootGuard::with_inspection_request(root, &foreign, |_| {
                panic!("quarantine admitted inspection")
            });
        assert_eq!(result.err().as_deref(), Some(QUARANTINE_ERROR));
        assert_eq!(snapshot(root), retained);
    }

    #[test]
    fn preexisting_staging_object_is_not_removed_or_reused() {
        let scratch = initialized();
        let root = &scratch.0;
        let request = mutation(root);
        let guard = DataRootGuard::open_existing_request(root, &request).unwrap();
        fs::write(
            root.join("control").join(STAGING),
            b"foreign unresolved staging input",
        )
        .unwrap();
        let before = control_snapshot(root);
        assert_eq!(
            guard.arm_catalog_intent(&request).err().as_deref(),
            Some(QUARANTINE_ERROR)
        );
        assert_eq!(control_snapshot(root), before);
        assert!(!root.join("control").join(MARKER).exists());
    }

    #[test]
    fn different_drain_reason_cannot_publish_release_or_clear_intent() {
        let scratch = initialized();
        let root = &scratch.0;
        let request = mutation(root);
        let mut guard = DataRootGuard::open_existing_request(root, &request).unwrap();
        let intent = guard.arm_catalog_intent(&request).unwrap();
        guard
            .begin_drain(search_runtime_owner::DrainReason::Restart)
            .unwrap();
        let before = control_snapshot(root);
        assert_eq!(
            guard
                .release_catalog_cleanly(intent, &request)
                .err()
                .as_deref(),
            Some("OWNER_GUARD_MISMATCH")
        );
        assert_eq!(control_snapshot(root), before);
        let current = super::super::slots::newest_valid(root).unwrap().1.unwrap();
        assert_eq!(current.lifecycle, LifecycleState::Draining);
        assert_eq!(current.drain_reason, DrainReasonText::Restart);
        assert_eq!(current.generation, 5);
        assert!(root.join("control").join(MARKER).exists());
        assert!(root.join("control").join(STAGING).exists());
    }
}

impl core::fmt::Debug for CatalogMutationIntent {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("CatalogMutationIntent(<opaque>)")
    }
}

fn input_value(owner: &LiveOwner, request: &DataRootRequest) -> Result<CanonicalValue, String> {
    request.preflight()?;
    owner
        .verify_existing()
        .map_err(|error| error.code().to_owned())?;
    if owner.record.lifecycle != LifecycleState::Active {
        return Err(OwnerError::OwnerInvalidTransition.code().to_owned());
    }
    let values = vec![
        CanonicalValue::Text(
            CanonicalText::new_non_empty(FORMAT).map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?,
        ),
        CanonicalValue::U64(2),
        CanonicalValue::Bytes(
            BoundedBytes::new(owner.record.encode())
                .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?,
        ),
        request.retained_input_value()?,
    ];
    Ok(CanonicalValue::Array(
        BoundedList::new(values).map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?,
    ))
}

impl CatalogMutationIntent {
    pub(super) fn arm(owner: &LiveOwner, request: &DataRootRequest) -> Result<Self, String> {
        let expected = input_value(owner, request)?;
        let encoded = to_canonical_cbor(&expected).map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let bytes = encoded.as_slice().to_vec();
        if bytes.len() > MAX_INTENT_BYTES {
            return Err(QUARANTINE_ARM_FAILED.to_owned());
        }
        let root = &owner.canonical_root;
        let control = root.join("control");
        let staging = control.join(STAGING);
        let marker = control.join(MARKER);
        // A prior final or staging object is unresolved evidence, even if its
        // bytes resemble this invocation. Never remove or reuse it here.
        for path in [&staging, &marker] {
            request.preflight()?;
            let metadata = fs::symlink_metadata(path);
            request.preflight()?;
            if !matches!(metadata, Err(error) if error.kind() == io::ErrorKind::NotFound) {
                return Err(QUARANTINE_ERROR.to_owned());
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x0020_0000).share_mode(0x3);
        }
        request.preflight()?;
        let created = options.open(&staging);
        // Once creation may have started, cancellation is not a no-effect
        // refusal. Retain every name; the source closure has not been called.
        request
            .preflight()
            .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let mut file = created.map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        request
            .preflight()
            .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        written.map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        verify_existing_locator(&file, &staging).map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let linked = fs::hard_link(&staging, &marker);
        request
            .preflight()
            .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        linked.map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let synced = file.sync_all();
        request
            .preflight()
            .map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        synced.map_err(|_| QUARANTINE_ARM_FAILED.to_owned())?;
        let intent = Self {
            root: root.clone(),
            file: Some(file),
            bytes,
            request: request.retain(),
            owner_record: owner.record.clone(),
            value: expected,
        };
        intent.verify(owner, request)?;
        Ok(intent)
    }

    fn verify(&self, owner: &LiveOwner, request: &DataRootRequest) -> Result<(), String> {
        request.preflight()?;
        if self.root != owner.canonical_root || !self.request.same_context(request) {
            return Err(OwnerError::OwnerOperationConflict.code().to_owned());
        }
        owner
            .verify_existing()
            .map_err(|error| error.code().to_owned())?;
        // Compare the actual retained owner, not a caller-supplied epoch or
        // receipt. Drain retains Shutdown; the actual RELEASED transition resets
        // the reason to None. Validate the drain before publishing that release.
        let mut expected_owner = self.owner_record.clone();
        if matches!(
            owner.record.lifecycle,
            LifecycleState::Draining | LifecycleState::Released
        ) {
            expected_owner.lifecycle = owner.record.lifecycle;
            expected_owner.drain_reason = if owner.record.lifecycle == LifecycleState::Draining {
                DrainReasonText::Shutdown
            } else {
                DrainReasonText::None
            };
            expected_owner.generation = expected_owner
                .generation
                .checked_add(if owner.record.lifecycle == LifecycleState::Draining {
                    1
                } else {
                    2
                })
                .ok_or_else(|| OwnerError::ContractExhausted.code().to_owned())?;
            // The actual digest was independently verified by the live owner;
            // no alternate hash or manufactured durable record is published.
            expected_owner.record_digest = owner.record.record_digest;
        }
        if owner.record != expected_owner {
            return Err(OwnerError::OwnerGuardMismatch.code().to_owned());
        }
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| QUARANTINE_ERROR.to_owned())?;
        for name in [STAGING, MARKER] {
            let path = self.root.join("control").join(name);
            verify_existing_locator(file, &path).map_err(|_| QUARANTINE_ERROR.to_owned())?;
            request.preflight()?;
            let observed = read_existing_bytes(&path, MAX_INTENT_BYTES);
            request.preflight()?;
            let observed = observed
                .map_err(|_| QUARANTINE_ERROR.to_owned())?
                .ok_or_else(|| QUARANTINE_ERROR.to_owned())?;
            if observed != self.bytes
                || parse_canonical_cbor(&observed).map_err(|_| QUARANTINE_ERROR.to_owned())?
                    != self.value
            {
                return Err(QUARANTINE_ERROR.to_owned());
            }
        }
        request.preflight()
    }

    pub(super) fn verify_for_release(
        &self,
        owner: &LiveOwner,
        request: &DataRootRequest,
    ) -> Result<(), String> {
        if owner.record.lifecycle != LifecycleState::Draining {
            return Err(OwnerError::OwnerDrainRequired.code().to_owned());
        }
        self.verify(owner, request)
    }

    pub(super) fn clear(
        mut self,
        owner: &LiveOwner,
        request: &DataRootRequest,
    ) -> Result<(), String> {
        if owner.record.lifecycle != LifecycleState::Released {
            return Err(OwnerError::OwnerInvalidTransition.code().to_owned());
        }
        self.verify(owner, request)?;
        // Windows held sharing denied replacement throughout dispatch/readback.
        // Closing precedes unlinking while the same native root owner remains held.
        drop(self.file.take());
        for name in [MARKER, STAGING] {
            request.preflight()?;
            let path = self.root.join("control").join(name);
            let observed = read_existing_bytes(&path, MAX_INTENT_BYTES);
            request.preflight()?;
            if observed.map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())? != Some(self.bytes.clone())
            {
                return Err(QUARANTINE_ERROR.to_owned());
            }
            let removed = fs::remove_file(&path);
            request
                .preflight()
                .map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
            removed.map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
            let metadata = fs::symlink_metadata(&path);
            request
                .preflight()
                .map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())?;
            if !matches!(metadata, Err(error) if error.kind() == io::ErrorKind::NotFound) {
                return Err(QUARANTINE_CLEAR_FAILED.to_owned());
            }
        }
        owner
            .verify_existing()
            .map_err(|error| error.code().to_owned())?;
        request
            .preflight()
            .map_err(|_| QUARANTINE_CLEAR_FAILED.to_owned())
    }
}
