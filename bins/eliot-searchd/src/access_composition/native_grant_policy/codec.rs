//! Closed binary policy-row schema; no credentials, claims or source payloads.

use search_contracts::{
    AccessPartitionId, BindingId, BoundedSet, CorpusId, CorpusOrPortfolioId,
    DisclosureCeiling, InstallationId, InstallationIncarnationId, Modality, OpaqueId,
    OpaqueRef, PortfolioRevision, ProfileId, RecipeIdV1, ReferencePortfolioId, ScopeDomainId,
    SensitivityClass, SourceMembershipId, UtcTimestamp, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS,
};
use search_control_redb::{ControlRecordClass, ControlValue, JournalLimits};

use super::{AuthoritativeGrantPolicy, NativeGrantPolicyError, StandalonePolicyRecord, StandalonePolicyState};

type Result<T> = std::result::Result<T, NativeGrantPolicyError>;
const MAGIC: &[u8; 8] = b"ELGRPOL1";
const LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;
const INVALID: NativeGrantPolicyError = NativeGrantPolicyError::InvalidRecord;

pub(super) fn encode(record: &StandalonePolicyRecord) -> Result<ControlValue> {
    record.validate()?;
    let mut out = Writer(Vec::new());
    out.raw(MAGIC)?;
    out.byte(match record.state {
        StandalonePolicyState::Active => 0,
        StandalonePolicyState::Revoked => 1,
        StandalonePolicyState::Expired => 2,
    })?;
    out.text(record.issued_at.as_str())?;
    out.byte(u8::from(record.expires_at.is_some()))?;
    if let Some(value) = &record.expires_at { out.text(value.as_str())?; }
    let p = &record.policy;
    out.raw(p.binding_id.as_bytes())?;
    out.u64(p.binding_generation)?;
    out.u64(p.policy_generation)?;
    out.raw(p.installation_id.as_bytes())?;
    out.raw(p.installation_incarnation_id.as_bytes())?;
    out.text(p.principal_opaque_id.as_str())?;
    out.text(p.client_scope_ref.as_str())?;
    out.raw(p.scope_domain_id.as_bytes())?;
    out.set(&p.allowed_membership_ids, |out, id| out.raw(id.as_bytes()))?;
    out.set(&p.allowed_corpus_or_portfolio_ids, |out, id| match id {
        CorpusOrPortfolioId::Corpus(id) => { out.byte(0)?; out.raw(id.as_bytes()) }
        CorpusOrPortfolioId::Portfolio(id) => { out.byte(1)?; out.raw(id.as_bytes()) }
    })?;
    out.byte(u8::from(p.reference_portfolio_revision.is_some()))?;
    if let Some(value) = p.reference_portfolio_revision { out.u64(value.get())?; }
    out.set(&p.allowed_access_partitions, |out, id| out.raw(id.as_bytes()))?;
    out.set(&p.allowed_modalities, |out, value| out.text(value.as_str()))?;
    out.set(&p.permitted_recipe_families, |out, value| out.text(value.as_str()))?;
    out.set(&p.allowed_budget_classes, |out, value| out.text(value.as_str()))?;
    out.text(p.sensitivity_ceiling.as_str())?;
    out.text(p.disclosure_ceiling.as_str())?;
    out.byte(u8::from(p.source_read_permission))?;
    out.byte(u8::from(p.exact_scan_permission))?;
    out.text(p.issued_boot_id.as_str())?;
    out.u64(p.revocation_generation)?;
    out.u64(p.maximum_ttl_ms)?;
    ControlValue::new(ControlRecordClass::State, out.0, JournalLimits::BASELINE).map_err(Into::into)
}

pub(super) fn decode(value: &ControlValue) -> Result<StandalonePolicyRecord> {
    if value.class() != ControlRecordClass::State || value.is_empty() || value.len() > LIMIT {
        return Err(INVALID);
    }
    let mut input = Reader { bytes: value.as_bytes(), position: 0 };
    if input.take(MAGIC.len())? != MAGIC { return Err(INVALID); }
    let state = match input.byte()? {
        0 => StandalonePolicyState::Active,
        1 => StandalonePolicyState::Revoked,
        2 => StandalonePolicyState::Expired,
        _ => return Err(INVALID),
    };
    let issued_at = UtcTimestamp::parse(input.text(27)?).map_err(|_| INVALID)?;
    let expires_at = if input.boolean()? {
        Some(UtcTimestamp::parse(input.text(27)?).map_err(|_| INVALID)?)
    } else { None };
    let policy = AuthoritativeGrantPolicy {
        binding_id: BindingId::from_bytes(input.array()?),
        binding_generation: input.u64()?,
        policy_generation: input.u64()?,
        installation_id: InstallationId::from_bytes(input.array()?),
        installation_incarnation_id: InstallationIncarnationId::from_bytes(input.array()?),
        principal_opaque_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| INVALID)?,
        client_scope_ref: OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?).map_err(|_| INVALID)?,
        scope_domain_id: ScopeDomainId::from_bytes(input.array()?),
        allowed_membership_ids: input.set(|input| Ok(SourceMembershipId::from_bytes(input.array()?)))?,
        allowed_corpus_or_portfolio_ids: input.set(|input| match input.byte()? {
            0 => Ok(CorpusOrPortfolioId::Corpus(CorpusId::from_bytes(input.array()?))),
            1 => Ok(CorpusOrPortfolioId::Portfolio(ReferencePortfolioId::from_bytes(input.array()?))),
            _ => Err(INVALID),
        })?,
        reference_portfolio_revision: if input.boolean()? {
            Some(PortfolioRevision::new(input.u64()?))
        } else { None },
        allowed_access_partitions: input.set(|input| Ok(AccessPartitionId::from_bytes(input.array()?)))?,
        allowed_modalities: input.set(|input| Modality::parse(input.text(64)?).map_err(|_| INVALID))?,
        permitted_recipe_families: input.set(|input| RecipeIdV1::parse(input.text(64)?).map_err(|_| INVALID))?,
        allowed_budget_classes: input.set(|input| ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?).map_err(|_| INVALID))?,
        sensitivity_ceiling: SensitivityClass::parse(input.text(64)?).map_err(|_| INVALID)?,
        disclosure_ceiling: DisclosureCeiling::parse(input.text(64)?).map_err(|_| INVALID)?,
        source_read_permission: input.boolean()?,
        exact_scan_permission: input.boolean()?,
        issued_boot_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| INVALID)?,
        revocation_generation: input.u64()?,
        maximum_ttl_ms: input.u64()?,
    };
    if input.position != input.bytes.len() { return Err(INVALID); }
    let record = StandalonePolicyRecord { policy, state, issued_at, expires_at };
    record.validate()?;
    Ok(record)
}

struct Writer(Vec<u8>);
impl Writer {
    fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        if self.0.len().checked_add(bytes.len()).is_none_or(|total| total > LIMIT) {
            return Err(INVALID);
        }
        self.0.try_reserve(bytes.len()).map_err(|_| INVALID)?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, value: u8) -> Result<()> { self.raw(&[value]) }
    fn u64(&mut self, value: u64) -> Result<()> { self.raw(&value.to_be_bytes()) }
    fn text(&mut self, value: &str) -> Result<()> {
        let size = u32::try_from(value.len()).map_err(|_| INVALID)?;
        self.raw(&size.to_be_bytes())?;
        self.raw(value.as_bytes())
    }
    fn set<T: Ord>(
        &mut self,
        values: &BoundedSet<T, MAX_SET_ITEMS>,
        mut put: impl FnMut(&mut Self, &T) -> Result<()>,
    ) -> Result<()> {
        self.raw(&u32::try_from(values.len()).map_err(|_| INVALID)?.to_be_bytes())?;
        for value in values.iter() { put(self, value)?; }
        Ok(())
    }
}

struct Reader<'a> { bytes: &'a [u8], position: usize }
impl<'a> Reader<'a> {
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
        if length > maximum { return Err(INVALID); }
        Ok(length)
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.byte()? { 0 => Ok(false), 1 => Ok(true), _ => Err(INVALID) }
    }
    fn text(&mut self, maximum: usize) -> Result<&'a str> {
        let length = self.length(maximum)?;
        std::str::from_utf8(self.take(length)?).map_err(|_| INVALID)
    }
    fn set<T: Ord>(&mut self, mut get: impl FnMut(&mut Self) -> Result<T>) -> Result<BoundedSet<T, MAX_SET_ITEMS>> {
        let count = self.length(MAX_SET_ITEMS)?;
        // Every element in this schema occupies at least one byte. Reject the
        // announced count before allocating or iterating an impossible input.
        if count > self.bytes.len() - self.position { return Err(INVALID); }
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| INVALID)?;
        for _ in 0..count {
            let next = get(self)?;
            if values.last().is_some_and(|previous| previous >= &next) { return Err(INVALID); }
            values.push(next);
        }
        BoundedSet::from_items(values).map_err(|_| INVALID)
    }
}
