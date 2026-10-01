//! Read-only compatibility decoder for persisted provider binding rows.
//!
//! The authoritative encoder and write path live in `search-control-redb`.
//! This decoder remains only for non-standalone client-adapter reads and exact
//! historical provisioning evidence; it cannot mutate or publish authority.

use search_contracts::{
    BindingId, Blake3Digest32, BoundedSet, InstallationId, InstallationIncarnationId,
    NonZeroRevision, OpaqueRef, ProfileId, ProviderBindingRecord, ProviderBindingStatus,
    UtcTimestamp, MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS,
    protocol::PeerRole,
};
use search_control_redb::{ControlRecordClass, ControlValue, JournalLimits};

use super::{NativeBindingError, validate_binding_record};

type Result<T> = std::result::Result<T, NativeBindingError>;
const BAD: NativeBindingError = NativeBindingError::InvalidRecord;
const MAGIC: &[u8; 8] = b"ELBIND01";
const LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;

pub(super) fn decode(value: &ControlValue) -> Result<ProviderBindingRecord> {
    if value.class() != ControlRecordClass::Identity
        || value.is_empty()
        || value.len() > LIMIT
    {
        return Err(BAD);
    }
    let mut input = Input(value.as_bytes());
    if input.take(MAGIC.len())? != MAGIC {
        return Err(BAD);
    }
    let binding_id = BindingId::from_bytes(input.array()?);
    let installation_id = InstallationId::from_bytes(input.array()?);
    let installation_incarnation_id = InstallationIncarnationId::from_bytes(input.array()?);
    let peer_role = match input.byte()? {
        0 => PeerRole::StandaloneCli,
        1 => PeerRole::ClientAdapter,
        _ => return Err(BAD),
    };
    let peer_identity_digest = Blake3Digest32::from_bytes(input.array()?);
    let pairing_generation = NonZeroRevision::new(input.u64()?).map_err(|_| BAD)?;
    let count = input.length(MAX_SET_ITEMS)?;
    // A nonempty profile has a four-byte length and at least one UTF-8 byte.
    if count > input.remaining().len() / 5 {
        return Err(BAD);
    }
    let mut profiles = Vec::new();
    profiles.try_reserve_exact(count).map_err(|_| BAD)?;
    for _ in 0..count {
        let profile = ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?).map_err(|_| BAD)?;
        if profiles.last().is_some_and(|previous| previous >= &profile) {
            return Err(BAD);
        }
        profiles.push(profile);
    }
    let permitted_profile_ids = BoundedSet::from_items(profiles).map_err(|_| BAD)?;
    let disclosure_ceiling_ref =
        OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?).map_err(|_| BAD)?;
    let issued_at = input.timestamp()?;
    let expires_at = match input.byte()? {
        0 => None,
        1 => Some(input.timestamp()?),
        _ => return Err(BAD),
    };
    let revocation_generation = NonZeroRevision::new(input.u64()?).map_err(|_| BAD)?;
    let status = match input.byte()? {
        0 => ProviderBindingStatus::Active,
        1 => ProviderBindingStatus::Revoked,
        2 => ProviderBindingStatus::Expired,
        _ => return Err(BAD),
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
    validate_binding_record(&record)?;
    Ok(record)
}

struct Input<'a>(&'a [u8]);

impl<'a> Input<'a> {
    fn remaining(&self) -> &'a [u8] { self.0 }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let value = self.0.get(..count).ok_or(BAD)?;
        self.0 = &self.0[count..];
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| BAD)
    }

    fn byte(&mut self) -> Result<u8> { Ok(self.array::<1>()?[0]) }

    fn u64(&mut self) -> Result<u64> { Ok(u64::from_be_bytes(self.array()?)) }

    fn length(&mut self, limit: usize) -> Result<usize> {
        let size = usize::try_from(u32::from_be_bytes(self.array()?)).map_err(|_| BAD)?;
        if size > limit {
            return Err(BAD);
        }
        Ok(size)
    }

    fn text(&mut self, limit: usize) -> Result<&'a str> {
        let count = self.length(limit)?;
        std::str::from_utf8(self.take(count)?).map_err(|_| BAD)
    }

    fn timestamp(&mut self) -> Result<UtcTimestamp> {
        UtcTimestamp::parse(std::str::from_utf8(self.take(27)?).map_err(|_| BAD)?)
            .map_err(|_| BAD)
    }

    fn finish(self) -> Result<()> {
        if self.0.is_empty() { Ok(()) } else { Err(BAD) }
    }
}
