//! Native retained evidence inspection. No succession, ordinary store or cleanup.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use search_contracts::{RequestId, parse_canonical_cbor};
use search_runtime_owner::OwnerError;

use super::catalog_intent::MAX_INTENT_BYTES;
use super::catalog_intent_decode::{
    CatalogIntentEvidence, CatalogOperationKind, CatalogRecoveryState,
};
use super::inspection::{ExistingOwnerSnapshot, inspect_existing_owner};
use super::installation::retain_native_installation;
use super::native_bindings::NativeLayoutPins;
use super::operation::DataRootRequest;
use super::read_existing::{read_existing_bytes, verify_existing_locator};
use super::slots::newest_valid;
use crate::development::DataRootGuard;

const COMMAND: &str = "--inspect-catalog-recovery";
const DISCOVERY_COMMAND: &str = "--list-catalog-recovery";
const MARKER: &str = "control/catalog-quarantine.marker";
const STAGING: &str = "control/catalog-quarantine.tmp";

/// An exact invocation name; parsing it never grants root authority.
pub struct CatalogRecoveryRequest(RequestId);

impl CatalogRecoveryRequest {
    pub(crate) fn parse(value: &str) -> Result<Self, OwnerError> {
        RequestId::parse(value)
            .map(Self)
            .map_err(|_| OwnerError::OwnerOperationConflict)
    }

    fn validate_inputs(&self, root: &Path, request: &DataRootRequest) -> Result<(), String> {
        request.validate_root(root)?;
        request.validate_cli_inputs(&[
            COMMAND.into(),
            root.as_os_str().to_owned(),
            self.0.to_string().into(),
        ])
    }
}

/// Content-free observations only; this does not authorize cleanup or a retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogRecoveryInspection {
    pub(crate) operation_id: RequestId,
    pub(crate) kind: CatalogOperationKind,
    pub(crate) state: CatalogRecoveryState,
    pub(crate) owner_epoch: u64,
    pub(crate) owner_generation: u64,
}

/// Non-cloneable native capability; construction is confined to exact admission.
struct CatalogRecovery {
    root: PathBuf,
    request: DataRootRequest,
    native_objects: NativeLayoutPins,
    snapshot: ExistingOwnerSnapshot,
    file: File,
    bytes: Vec<u8>,
    evidence: CatalogIntentEvidence,
    inspection: CatalogRecoveryInspection,
}

#[derive(Clone, Copy)]
enum CatalogRecoverySelection {
    Named(RequestId),
    Discover,
}

impl CatalogRecovery {
    fn verify(&self) -> Result<(), String> {
        self.request.preflight()?;
        self.native_objects.verify(&self.root).map_err(code)?;
        self.request.preflight()?;
        self.snapshot.verify_unchanged(&self.root).map_err(code)?;
        self.request.preflight()?;
        require_catalog_profile(&self.root, &self.request)?;
        for name in [MARKER, STAGING] {
            let path = self.root.join(name);
            verify_existing_locator(&self.file, &path).map_err(code)?;
            self.request.preflight()?;
            let observed = read_existing_bytes(&path, MAX_INTENT_BYTES);
            self.request.preflight()?;
            let observed = observed
                .map_err(code)?
                .ok_or_else(|| code(OwnerError::OwnerRecoveryEvidenceMissing))?;
            if observed != self.bytes
                || parse_canonical_cbor(&observed)
                    .map_err(|_| code(OwnerError::OwnerRecoveryQuarantined))?
                    != self.evidence.value
            {
                return Err(code(OwnerError::OwnerGuardMismatch));
            }
        }
        self.request.preflight()
    }
}

pub fn inspect_catalog_recovery_request(
    root: &Path,
    name: &CatalogRecoveryRequest,
    request: &DataRootRequest,
) -> Result<CatalogRecoveryInspection, String> {
    name.validate_inputs(root, request)?;
    inspect_catalog_recovery(root, CatalogRecoverySelection::Named(name.0), request)
}

/// Discovers only the one fixed retained intent; no directory inventory or authority.
pub fn list_catalog_recovery_request(
    root: &Path,
    request: &DataRootRequest,
) -> Result<CatalogRecoveryInspection, String> {
    request.validate_root(root)?;
    request.validate_cli_inputs(&[
        DISCOVERY_COMMAND.into(),
        root.as_os_str().to_owned(),
    ])?;
    inspect_catalog_recovery(root, CatalogRecoverySelection::Discover, request)
}

fn inspect_catalog_recovery(
    root: &Path,
    selection: CatalogRecoverySelection,
    request: &DataRootRequest,
) -> Result<CatalogRecoveryInspection, String> {
    let result = DataRootGuard::with_existing_lock(root, |canonical| {
        request.preflight()?;
        require_catalog_profile(canonical, request)?;
        request.preflight()?;
        let native_objects = retain_native_installation(canonical);
        request.preflight()?;
        let native_objects = native_objects.map_err(code)?;
        let snapshot = inspect_existing_owner(canonical);
        request.preflight()?;
        let snapshot = snapshot.map_err(code)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x0020_0000).share_mode(0x1);
        }
        let file = options.open(canonical.join(MARKER));
        request.preflight()?;
        let file = file.map_err(|_| code(OwnerError::OwnerRecoveryEvidenceMissing))?;
        for locator in [MARKER, STAGING] {
            verify_existing_locator(&file, &canonical.join(locator)).map_err(code)?;
            request.preflight()?;
        }
        let bytes = read_existing_bytes(&canonical.join(MARKER), MAX_INTENT_BYTES);
        request.preflight()?;
        let bytes = bytes
            .map_err(code)?
            .ok_or_else(|| code(OwnerError::OwnerRecoveryEvidenceMissing))?;
        let evidence = CatalogIntentEvidence::decode(&bytes).map_err(code)?;
        request.preflight()?;
        match selection {
            CatalogRecoverySelection::Named(id) if evidence.request_id != id => {
                return Err(code(OwnerError::OwnerOperationConflict));
            }
            CatalogRecoverySelection::Named(_) | CatalogRecoverySelection::Discover => {}
        }
        let current = newest_valid(canonical);
        request.preflight()?;
        let (_, current) = current.map_err(code)?;
        let current = current.ok_or_else(|| code(OwnerError::OwnerRecoveryEvidenceMissing))?;
        if !snapshot.matches_record(&current) {
            return Err(code(OwnerError::OwnerGuardMismatch));
        }
        let state = evidence.matches_current(&current).map_err(code)?;
        let inspection = CatalogRecoveryInspection {
            operation_id: evidence.request_id,
            kind: evidence.kind,
            state,
            owner_epoch: current.epoch,
            owner_generation: current.generation,
        };
        let cap = CatalogRecovery {
            root: canonical.to_owned(),
            request: request.retain(),
            native_objects,
            snapshot,
            file,
            bytes,
            evidence,
            inspection,
        };
        cap.verify()?;
        let result = cap.inspection;
        cap.verify()?;
        Ok(result)
    });
    request.preflight()?;
    result
}

fn require_catalog_profile(root: &Path, request: &DataRootRequest) -> Result<(), String> {
    for name in [
        ".eliot-search-initialization-intent.v1",
        "control/control.redb",
    ] {
        request.preflight()?;
        let observed = fs::symlink_metadata(root.join(name));
        request.preflight()?;
        if !matches!(observed, Err(error) if error.kind() == io::ErrorKind::NotFound) {
            return Err(code(OwnerError::OwnerRecoveryQuarantined));
        }
    }
    Ok(())
}

fn code(error: OwnerError) -> String {
    error.code().to_owned()
}

#[cfg(all(test, windows))]
#[path = "catalog_recovery_tests.rs"]
mod tests;
