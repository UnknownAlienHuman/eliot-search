//! Read-only original catalog-intent evidence. Decoding grants no authority.

use search_contracts::{
    CanonicalDigestDomain, CanonicalValue, DigestInputLimit, RequestId, blake3_canonical,
    parse_canonical_cbor,
};
use search_runtime_owner::OwnerError;

use super::catalog_intent::{FORMAT, MAX_INTENT_BYTES};
use super::operation::DataRootRequest;
use super::record::DurableOwnerRecord;
use super::spec::{DrainReasonText, LifecycleState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogOperationKind {
    IndexFile,
    IndexDirectory,
    RetireSource,
    GcApply,
}

impl CatalogOperationKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::IndexFile => "index_file",
            Self::IndexDirectory => "index_directory",
            Self::RetireSource => "retire_source",
            Self::GcApply => "gc_apply",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogRecoveryState {
    UnresolvedActive,
    UnresolvedDraining,
    ReleasedAwaitingCleanup,
}

impl CatalogRecoveryState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnresolvedActive => "unresolved_active",
            Self::UnresolvedDraining => "unresolved_draining",
            Self::ReleasedAwaitingCleanup => "released_awaiting_cleanup",
        }
    }
}

pub(super) struct CatalogIntentEvidence {
    pub(super) original_record: DurableOwnerRecord,
    pub(super) request_id: RequestId,
    pub(super) kind: CatalogOperationKind,
    // Retain the complete original canonical value, not only its digest/id.
    pub(super) value: CanonicalValue,
}

impl CatalogIntentEvidence {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, OwnerError> {
        if bytes.len() > MAX_INTENT_BYTES {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let value =
            parse_canonical_cbor(bytes).map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let CanonicalValue::Array(fields) = &value else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
        let [
            CanonicalValue::Text(tag),
            CanonicalValue::U64(2),
            CanonicalValue::Bytes(record),
            CanonicalValue::Array(input),
        ] = fields.as_slice()
        else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
        if tag.as_str() != FORMAT {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let original_record = DurableOwnerRecord::decode(record.as_slice())?;
        if original_record.lifecycle != LifecycleState::Active
            || original_record.encode() != record.as_slice()
        {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let [
            CanonicalValue::Text(domain),
            id,
            digest,
            payload @ CanonicalValue::Array(_),
        ] = input.as_slice()
        else {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        };
        if domain.as_str() != DataRootRequest::cli_input_domain() {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        let request_id = RequestId::from_bytes(fixed_bytes(id)?);
        let original_digest: [u8; 32] = fixed_bytes(digest)?;
        let kind = original_command(payload)?;
        let domain = CanonicalDigestDomain::parse(domain.as_str())
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let limit = DigestInputLimit::new(DataRootRequest::cli_digest_byte_limit())
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        let observed = blake3_canonical(&domain, payload, limit)
            .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
        if observed.as_bytes() != &original_digest {
            return Err(OwnerError::OwnerRecoveryQuarantined);
        }
        Ok(Self {
            original_record,
            request_id,
            kind,
            value,
        })
    }

    /// Exact same-owner lifecycle observation, never an operation-success proof.
    pub(super) fn matches_current(
        &self,
        current: &DurableOwnerRecord,
    ) -> Result<CatalogRecoveryState, OwnerError> {
        current.validate_shape()?;
        let (state, steps, reason) = match current.lifecycle {
            LifecycleState::Active => (
                CatalogRecoveryState::UnresolvedActive,
                0,
                DrainReasonText::None,
            ),
            LifecycleState::Draining => (
                CatalogRecoveryState::UnresolvedDraining,
                1,
                DrainReasonText::Shutdown,
            ),
            LifecycleState::Released => (
                CatalogRecoveryState::ReleasedAwaitingCleanup,
                2,
                DrainReasonText::None,
            ),
        };
        let mut expected = self.original_record.clone();
        expected.lifecycle = current.lifecycle;
        expected.drain_reason = reason;
        expected.generation = expected
            .generation
            .checked_add(steps)
            .ok_or(OwnerError::ContractExhausted)?;
        // The caller independently validates the actual current native owner;
        // this compares all other fields without publishing an alternate record.
        if steps != 0 {
            expected.record_digest = current.record_digest;
        }
        if &expected != current {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        Ok(state)
    }
}

fn fixed_bytes<const N: usize>(value: &CanonicalValue) -> Result<[u8; N], OwnerError> {
    let CanonicalValue::Bytes(bytes) = value else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)
}

fn original_command(payload: &CanonicalValue) -> Result<CatalogOperationKind, OwnerError> {
    let CanonicalValue::Array(arguments) = payload else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    let [
        CanonicalValue::Bytes(command),
        CanonicalValue::Bytes(root),
        CanonicalValue::Bytes(argument),
    ] = arguments.as_slice()
    else {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    };
    let total = [command, root, argument]
        .into_iter()
        .try_fold(0_usize, |total, bytes| total.checked_add(bytes.len()))
        .ok_or(OwnerError::ContractExhausted)?;
    if root.is_empty() || total > DataRootRequest::cli_input_byte_limit() {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    #[cfg(windows)]
    if [command, root, argument]
        .into_iter()
        .any(|bytes| bytes.len() % 2 != 0)
    {
        return Err(OwnerError::OwnerRecoveryQuarantined);
    }
    if command.as_slice() == native_flag("--index-file") {
        Ok(CatalogOperationKind::IndexFile)
    } else if command.as_slice() == native_flag("--index-directory") {
        Ok(CatalogOperationKind::IndexDirectory)
    } else if command.as_slice() == native_flag("--retire-source") {
        Ok(CatalogOperationKind::RetireSource)
    } else if command.as_slice() == native_flag("--gc-root")
        && argument.as_slice() == native_flag("--apply")
    {
        Ok(CatalogOperationKind::GcApply)
    } else {
        Err(OwnerError::OwnerRecoveryQuarantined)
    }
}

fn native_flag(value: &str) -> Vec<u8> {
    #[cfg(windows)]
    {
        value.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }
    #[cfg(not(windows))]
    {
        value.as_bytes().to_vec()
    }
}
