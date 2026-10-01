//! Read-only compatibility decoder for persisted standalone policy rows.
//!
//! The authoritative encoder and write path live in `search-control-redb`.
//! This decoder remains only for exact historical provisioning evidence and
//! cannot mutate, publish or widen grant authority.

use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, InstallationId, InstallationIncarnationId, Modality, OpaqueId,
    OpaqueRef, PortfolioRevision, ProfileId, RecipeIdV1, ReferencePortfolioId, ScopeDomainId,
    SensitivityClass, SourceMembershipId, UtcTimestamp, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS,
};
use search_control_redb::{ControlRecordClass, ControlValue, JournalLimits};

use super::{
    AuthoritativeGrantPolicy, NativeGrantPolicyError, StandalonePolicyRecord,
    StandalonePolicyState, validate_policy_record,
};

type Result<T> = std::result::Result<T, NativeGrantPolicyError>;
const MAGIC: &[u8; 8] = b"ELGRPOL1";
const LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;
const INVALID: NativeGrantPolicyError = NativeGrantPolicyError::InvalidRecord;

pub(super) fn decode(value: &ControlValue) -> Result<StandalonePolicyRecord> {
    if value.class() != ControlRecordClass::State
        || value.is_empty()
        || value.len() > LIMIT
    {
        return Err(INVALID);
    }
    let mut input = Reader { bytes: value.as_bytes(), position: 0 };
    if input.take(MAGIC.len())? != MAGIC {
        return Err(INVALID);
    }
    let state = match input.byte()? {
        0 => StandalonePolicyState::Active,
        1 => StandalonePolicyState::Revoked,
        2 => StandalonePolicyState::Expired,
        _ => return Err(INVALID),
    };
    let issued_at = UtcTimestamp::parse(input.text(27)?).map_err(|_| INVALID)?;
    let expires_at = if input.boolean()? {
        Some(UtcTimestamp::parse(input.text(27)?).map_err(|_| INVALID)?)
    } else {
        None
    };
    let policy = AuthoritativeGrantPolicy {
        binding_id: BindingId::from_bytes(input.array()?),
        binding_generation: input.u64()?,
        policy_generation: input.u64()?,
        installation_id: InstallationId::from_bytes(input.array()?),
        installation_incarnation_id: InstallationIncarnationId::from_bytes(input.array()?),
        principal_opaque_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?)
            .map_err(|_| INVALID)?,
        client_scope_ref: OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
            .map_err(|_| INVALID)?,
        scope_domain_id: ScopeDomainId::from_bytes(input.array()?),
        allowed_membership_ids: input.set(|input| {
            Ok(SourceMembershipId::from_bytes(input.array()?))
        })?,
        allowed_corpus_or_portfolio_ids: input.set(|input| match input.byte()? {
            0 => Ok(CorpusOrPortfolioId::Corpus(CorpusId::from_bytes(
                input.array()?,
            ))),
            1 => Ok(CorpusOrPortfolioId::Portfolio(
                ReferencePortfolioId::from_bytes(input.array()?),
            )),
            _ => Err(INVALID),
        })?,
        reference_portfolio_revision: if input.boolean()? {
            Some(PortfolioRevision::new(input.u64()?))
        } else {
            None
        },
        allowed_access_partitions: input.set(|input| {
            Ok(AccessPartitionId::from_bytes(input.array()?))
        })?,
        allowed_modalities: input.set(|input| {
            Modality::parse(input.text(64)?).map_err(|_| INVALID)
        })?,
        permitted_recipe_families: input.set(|input| {
            RecipeIdV1::parse(input.text(64)?).map_err(|_| INVALID)
        })?,
        allowed_budget_classes: input.set(|input| {
            ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?).map_err(|_| INVALID)
        })?,
        sensitivity_ceiling: SensitivityClass::parse(input.text(64)?).map_err(|_| INVALID)?,
        disclosure_ceiling: DisclosureCeiling::parse(input.text(64)?).map_err(|_| INVALID)?,
        source_read_permission: input.boolean()?,
        exact_scan_permission: input.boolean()?,
        issued_boot_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| INVALID)?,
        revocation_generation: input.u64()?,
        maximum_ttl_ms: input.u64()?,
    };
    input.finish()?;
    let record = StandalonePolicyRecord {
        policy,
        state,
        issued_at,
        expires_at,
    };
    validate_policy_record(&record)?;
    Ok(record)
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> &'a [u8] { &self.bytes[self.position..] }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(length).ok_or(INVALID)?;
        let bytes = self.bytes.get(self.position..end).ok_or(INVALID)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| INVALID)
    }

    fn byte(&mut self) -> Result<u8> { Ok(self.array::<1>()?[0]) }

    fn u64(&mut self) -> Result<u64> { Ok(u64::from_be_bytes(self.array()?)) }

    fn length(&mut self, maximum: usize) -> Result<usize> {
        let length = usize::try_from(u32::from_be_bytes(self.array()?)).map_err(|_| INVALID)?;
        if length > maximum {
            return Err(INVALID);
        }
        Ok(length)
    }

    fn boolean(&mut self) -> Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(INVALID),
        }
    }

    fn text(&mut self, maximum: usize) -> Result<&'a str> {
        let length = self.length(maximum)?;
        std::str::from_utf8(self.take(length)?).map_err(|_| INVALID)
    }

    fn set<T: Ord>(
        &mut self,
        mut get: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<BoundedSet<T, MAX_SET_ITEMS>> {
        let count = self.length(MAX_SET_ITEMS)?;
        // Every element in this schema occupies at least one byte. Reject an
        // impossible announced count before allocation or iteration.
        if count > self.remaining().len() {
            return Err(INVALID);
        }
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| INVALID)?;
        for _ in 0..count {
            let next = get(self)?;
            if values.last().is_some_and(|previous| previous >= &next) {
                return Err(INVALID);
            }
            values.push(next);
        }
        BoundedSet::from_items(values).map_err(|_| INVALID)
    }

    fn finish(self) -> Result<()> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(INVALID)
        }
    }
}
