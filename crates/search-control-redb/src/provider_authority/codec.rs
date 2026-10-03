//! Strict private codecs for provider binding and standalone policy rows.

use search_contracts::{
    AccessPartitionId, AuthoritativeGrantPolicy, BindingId, Blake3Digest32, BoundedSet,
    CorpusId, CorpusOrPortfolioId, DisclosureCeiling, InstallationId,
    InstallationIncarnationId, Modality, NonZeroRevision, OpaqueId, OpaqueRef,
    PortfolioRevision, ProfileId, ProviderBindingRecord, ProviderBindingStatus, RecipeIdV1,
    ReferencePortfolioId, ScopeDomainId, SensitivityClass, SourceMembershipId,
    StandalonePolicyRecord, StandalonePolicyState, UtcTimestamp, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS, protocol::PeerRole,
};

use crate::{ControlError, ControlRecordClass, ControlValue, JournalLimits};

const BINDING_MAGIC: &[u8; 8] = b"ELBIND01";
const POLICY_MAGIC: &[u8; 8] = b"ELGRPOL1";
const VALUE_LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;

pub(super) fn encode_binding(
    record: &ProviderBindingRecord,
) -> Result<ControlValue, ControlError> {
    record.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    let mut output = Vec::new();
    append(&mut output, BINDING_MAGIC, ControlError::InvalidValue)?;
    append(
        &mut output,
        record.binding_id.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        record.installation_id.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        record.installation_incarnation_id.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        &[match record.peer_role {
            PeerRole::StandaloneCli => 0,
            PeerRole::ClientAdapter => 1,
            _ => return Err(ControlError::InvalidValue),
        }],
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        record.peer_identity_digest.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        &record.pairing_generation.get().to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        &u32::try_from(record.permitted_profile_ids.len())
            .map_err(|_| ControlError::InvalidValue)?
            .to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    for profile in &record.permitted_profile_ids {
        write_text(
            &mut output,
            profile.as_str(),
            ControlError::InvalidValue,
        )?;
    }
    write_text(
        &mut output,
        record.disclosure_ceiling_ref.as_str(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        record.issued_at.as_str().as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        &[u8::from(record.expires_at.is_some())],
        ControlError::InvalidValue,
    )?;
    if let Some(expires_at) = &record.expires_at {
        append(
            &mut output,
            expires_at.as_str().as_bytes(),
            ControlError::InvalidValue,
        )?;
    }
    append(
        &mut output,
        &record.revocation_generation.get().to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut output,
        &[match record.status {
            ProviderBindingStatus::Active => 0,
            ProviderBindingStatus::Revoked => 1,
            ProviderBindingStatus::Expired => 2,
        }],
        ControlError::InvalidValue,
    )?;
    ControlValue::new(
        ControlRecordClass::Identity,
        output,
        JournalLimits::BASELINE,
    )
}

pub(super) fn decode_binding(
    value: &ControlValue,
) -> Result<ProviderBindingRecord, ControlError> {
    if value.class() != ControlRecordClass::Identity
        || value.is_empty()
        || value.len() > VALUE_LIMIT
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut input = Reader::new(value.as_bytes());
    if input.take(BINDING_MAGIC.len())? != BINDING_MAGIC {
        return Err(ControlError::StoreCorrupt);
    }
    let binding_id = BindingId::from_bytes(input.array()?);
    let installation_id = InstallationId::from_bytes(input.array()?);
    let installation_incarnation_id = InstallationIncarnationId::from_bytes(input.array()?);
    let peer_role = match input.byte()? {
        0 => PeerRole::StandaloneCli,
        1 => PeerRole::ClientAdapter,
        _ => return Err(ControlError::StoreCorrupt),
    };
    let peer_identity_digest = Blake3Digest32::from_bytes(input.array()?);
    let pairing_generation = NonZeroRevision::new(input.u64()?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let count = input.length(MAX_SET_ITEMS)?;
    if count > input.remaining().len() / 5 {
        return Err(ControlError::StoreCorrupt);
    }
    let mut profiles = Vec::new();
    profiles
        .try_reserve_exact(count)
        .map_err(|_| ControlError::StoreCorrupt)?;
    for _ in 0..count {
        let profile = ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?;
        if profiles.last().is_some_and(|previous| previous >= &profile) {
            return Err(ControlError::StoreCorrupt);
        }
        profiles.push(profile);
    }
    let permitted_profile_ids = BoundedSet::from_items(profiles)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let disclosure_ceiling_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let issued_at = input.timestamp()?;
    let expires_at = match input.byte()? {
        0 => None,
        1 => Some(input.timestamp()?),
        _ => return Err(ControlError::StoreCorrupt),
    };
    let revocation_generation = NonZeroRevision::new(input.u64()?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let status = match input.byte()? {
        0 => ProviderBindingStatus::Active,
        1 => ProviderBindingStatus::Revoked,
        2 => ProviderBindingStatus::Expired,
        _ => return Err(ControlError::StoreCorrupt),
    };
    input.finish()?;
    let record = ProviderBindingRecord {
        binding_id,
        installation_id,
        installation_incarnation_id,
        peer_role,
        peer_identity_digest,
        pairing_generation,
        permitted_profile_ids,
        disclosure_ceiling_ref,
        issued_at,
        expires_at,
        revocation_generation,
        status,
    };
    record
        .validate_shape()
        .map_err(|_| ControlError::StoreCorrupt)?;
    Ok(record)
}

pub(super) fn encode_policy(
    record: &StandalonePolicyRecord,
) -> Result<ControlValue, ControlError> {
    record.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    let mut output = Writer::new();
    output.raw(POLICY_MAGIC)?;
    output.byte(match record.state {
        StandalonePolicyState::Active => 0,
        StandalonePolicyState::Revoked => 1,
        StandalonePolicyState::Expired => 2,
    })?;
    output.text(record.issued_at.as_str())?;
    output.byte(u8::from(record.expires_at.is_some()))?;
    if let Some(expires_at) = &record.expires_at {
        output.text(expires_at.as_str())?;
    }
    let policy = &record.policy;
    output.raw(policy.binding_id.as_bytes())?;
    output.u64(policy.binding_generation)?;
    output.u64(policy.policy_generation)?;
    output.raw(policy.installation_id.as_bytes())?;
    output.raw(policy.installation_incarnation_id.as_bytes())?;
    output.text(policy.principal_opaque_id.as_str())?;
    output.text(policy.client_scope_ref.as_str())?;
    output.raw(policy.scope_domain_id.as_bytes())?;
    output.set(&policy.allowed_membership_ids, |writer, id| {
        writer.raw(id.as_bytes())
    })?;
    output.set(
        &policy.allowed_corpus_or_portfolio_ids,
        |writer, id| match id {
            CorpusOrPortfolioId::Corpus(id) => {
                writer.byte(0)?;
                writer.raw(id.as_bytes())
            }
            CorpusOrPortfolioId::Portfolio(id) => {
                writer.byte(1)?;
                writer.raw(id.as_bytes())
            }
        },
    )?;
    output.byte(u8::from(policy.reference_portfolio_revision.is_some()))?;
    if let Some(revision) = policy.reference_portfolio_revision {
        output.u64(revision.get())?;
    }
    output.set(&policy.allowed_access_partitions, |writer, id| {
        writer.raw(id.as_bytes())
    })?;
    output.set(&policy.allowed_modalities, |writer, modality| {
        writer.text(modality.as_str())
    })?;
    output.set(&policy.permitted_recipe_families, |writer, recipe| {
        writer.text(recipe.as_str())
    })?;
    output.set(&policy.allowed_budget_classes, |writer, profile| {
        writer.text(profile.as_str())
    })?;
    output.text(policy.sensitivity_ceiling.as_str())?;
    output.text(policy.disclosure_ceiling.as_str())?;
    output.byte(u8::from(policy.source_read_permission))?;
    output.byte(u8::from(policy.exact_scan_permission))?;
    output.text(policy.issued_boot_id.as_str())?;
    output.u64(policy.revocation_generation)?;
    output.u64(policy.maximum_ttl_ms)?;
    ControlValue::new(
        ControlRecordClass::State,
        output.finish(),
        JournalLimits::BASELINE,
    )
}

pub(super) fn decode_policy(
    value: &ControlValue,
) -> Result<StandalonePolicyRecord, ControlError> {
    if value.class() != ControlRecordClass::State
        || value.is_empty()
        || value.len() > VALUE_LIMIT
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut input = Reader::new(value.as_bytes());
    if input.take(POLICY_MAGIC.len())? != POLICY_MAGIC {
        return Err(ControlError::StoreCorrupt);
    }
    let state = match input.byte()? {
        0 => StandalonePolicyState::Active,
        1 => StandalonePolicyState::Revoked,
        2 => StandalonePolicyState::Expired,
        _ => return Err(ControlError::StoreCorrupt),
    };
    let issued_at = UtcTimestamp::parse(input.text(27)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let expires_at = if input.boolean()? {
        Some(
            UtcTimestamp::parse(input.text(27)?)
                .map_err(|_| ControlError::StoreCorrupt)?,
        )
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
            .map_err(|_| ControlError::StoreCorrupt)?,
        client_scope_ref: OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        scope_domain_id: ScopeDomainId::from_bytes(input.array()?),
        allowed_membership_ids: input.set(|reader| {
            Ok(SourceMembershipId::from_bytes(reader.array()?))
        })?,
        allowed_corpus_or_portfolio_ids: input.set(|reader| match reader.byte()? {
            0 => Ok(CorpusOrPortfolioId::Corpus(CorpusId::from_bytes(
                reader.array()?,
            ))),
            1 => Ok(CorpusOrPortfolioId::Portfolio(
                ReferencePortfolioId::from_bytes(reader.array()?),
            )),
            _ => Err(ControlError::StoreCorrupt),
        })?,
        reference_portfolio_revision: if input.boolean()? {
            Some(PortfolioRevision::new(input.u64()?))
        } else {
            None
        },
        allowed_access_partitions: input.set(|reader| {
            Ok(AccessPartitionId::from_bytes(reader.array()?))
        })?,
        allowed_modalities: input.set(|reader| {
            Modality::parse(reader.text(64)?).map_err(|_| ControlError::StoreCorrupt)
        })?,
        permitted_recipe_families: input.set(|reader| {
            RecipeIdV1::parse(reader.text(64)?).map_err(|_| ControlError::StoreCorrupt)
        })?,
        allowed_budget_classes: input.set(|reader| {
            ProfileId::new(reader.text(MAX_PROFILE_ID_BYTES)?)
                .map_err(|_| ControlError::StoreCorrupt)
        })?,
        sensitivity_ceiling: SensitivityClass::parse(input.text(64)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        disclosure_ceiling: DisclosureCeiling::parse(input.text(64)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        source_read_permission: input.boolean()?,
        exact_scan_permission: input.boolean()?,
        issued_boot_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
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
    record
        .validate_shape()
        .map_err(|_| ControlError::StoreCorrupt)?;
    Ok(record)
}

fn append(
    output: &mut Vec<u8>,
    bytes: &[u8],
    error: ControlError,
) -> Result<(), ControlError> {
    if output
        .len()
        .checked_add(bytes.len())
        .is_none_or(|end| end > VALUE_LIMIT)
    {
        return Err(error);
    }
    output.try_reserve(bytes.len()).map_err(|_| error)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn write_text(
    output: &mut Vec<u8>,
    value: &str,
    error: ControlError,
) -> Result<(), ControlError> {
    append(
        output,
        &u32::try_from(value.len())
            .map_err(|_| error)?
            .to_be_bytes(),
        error,
    )?;
    append(output, value.as_bytes(), error)
}

struct Writer(Vec<u8>);

impl Writer {
    const fn new() -> Self { Self(Vec::new()) }

    fn finish(self) -> Vec<u8> { self.0 }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), ControlError> {
        append(&mut self.0, bytes, ControlError::InvalidValue)
    }

    fn byte(&mut self, value: u8) -> Result<(), ControlError> { self.raw(&[value]) }

    fn u64(&mut self, value: u64) -> Result<(), ControlError> {
        self.raw(&value.to_be_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), ControlError> {
        write_text(&mut self.0, value, ControlError::InvalidValue)
    }

    fn set<T: Ord>(
        &mut self,
        values: &BoundedSet<T, MAX_SET_ITEMS>,
        mut put: impl FnMut(&mut Self, &T) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        self.raw(
            &u32::try_from(values.len())
                .map_err(|_| ControlError::InvalidValue)?
                .to_be_bytes(),
        )?;
        for value in values {
            put(self, value)?;
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self { Self { bytes, position: 0 } }

    fn remaining(&self) -> &'a [u8] { &self.bytes[self.position..] }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ControlError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ControlError::StoreCorrupt)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(ControlError::StoreCorrupt)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ControlError::StoreCorrupt)
    }

    fn byte(&mut self) -> Result<u8, ControlError> { Ok(self.array::<1>()?[0]) }

    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn length(&mut self, maximum: usize) -> Result<usize, ControlError> {
        let length = usize::try_from(u32::from_be_bytes(self.array()?))
            .map_err(|_| ControlError::StoreCorrupt)?;
        if length > maximum {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(length)
    }

    fn boolean(&mut self) -> Result<bool, ControlError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ControlError::StoreCorrupt),
        }
    }

    fn text(&mut self, maximum: usize) -> Result<&'a str, ControlError> {
        let length = self.length(maximum)?;
        core::str::from_utf8(self.take(length)?).map_err(|_| ControlError::StoreCorrupt)
    }

    fn timestamp(&mut self) -> Result<UtcTimestamp, ControlError> {
        UtcTimestamp::parse(
            core::str::from_utf8(self.take(27)?)
                .map_err(|_| ControlError::StoreCorrupt)?,
        )
        .map_err(|_| ControlError::StoreCorrupt)
    }

    fn set<T: Ord>(
        &mut self,
        mut get: impl FnMut(&mut Self) -> Result<T, ControlError>,
    ) -> Result<BoundedSet<T, MAX_SET_ITEMS>, ControlError> {
        let count = self.length(MAX_SET_ITEMS)?;
        if count > self.remaining().len() {
            return Err(ControlError::StoreCorrupt);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ControlError::StoreCorrupt)?;
        for _ in 0..count {
            let next = get(self)?;
            if values.last().is_some_and(|previous| previous >= &next) {
                return Err(ControlError::StoreCorrupt);
            }
            values.push(next);
        }
        BoundedSet::from_items(values).map_err(|_| ControlError::StoreCorrupt)
    }

    const fn finish(self) -> Result<(), ControlError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        }
    }
}
