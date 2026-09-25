//! Closed bounded persistence schema, not a provider message or key transcript.

use search_contracts::{BindingId, Blake3Digest32, BoundedSet, InstallationId,
    InstallationIncarnationId, NonZeroRevision, OpaqueRef, ProfileId, UtcTimestamp,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS, protocol::PeerRole};
use search_control_redb::{ControlRecordClass, ControlValue, JournalLimits};

use super::{NativeBindingError, ProviderBindingRecord, ProviderBindingStatus};

type Result<T> = std::result::Result<T, NativeBindingError>;
const BAD: NativeBindingError = NativeBindingError::InvalidRecord;
const MAGIC: &[u8; 8] = b"ELBIND01";
const LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;

pub(super) fn encode(record: &ProviderBindingRecord) -> Result<ControlValue> {
    record.validate()?;
    let mut out = Vec::new();
    append(&mut out, MAGIC)?;
    append(&mut out, record.binding_id.as_bytes())?;
    append(&mut out, record.installation_id.as_bytes())?;
    append(&mut out, record.installation_incarnation_id.as_bytes())?;
    append(&mut out, &[match record.peer_role {
        PeerRole::StandaloneCli => 0, PeerRole::ClientAdapter => 1, _ => return Err(BAD),
    }])?;
    append(&mut out, record.peer_identity_digest.as_bytes())?;
    append(&mut out, &record.pairing_generation.get().to_be_bytes())?;
    append(&mut out, &u32::try_from(record.permitted_profile_ids.len()).map_err(|_| BAD)?.to_be_bytes())?;
    for profile in record.permitted_profile_ids.iter() { text(&mut out, profile.as_str())?; }
    text(&mut out, record.disclosure_ceiling_ref.as_str())?;
    append(&mut out, record.issued_at.as_str().as_bytes())?;
    append(&mut out, &[u8::from(record.expires_at.is_some())])?;
    if let Some(expires) = &record.expires_at { append(&mut out, expires.as_str().as_bytes())?; }
    append(&mut out, &record.revocation_generation.get().to_be_bytes())?;
    append(&mut out, &[match record.status {
        ProviderBindingStatus::Active => 0,
        ProviderBindingStatus::Revoked => 1,
        ProviderBindingStatus::Expired => 2,
    }])?;
    ControlValue::new(ControlRecordClass::Identity, out, JournalLimits::BASELINE).map_err(Into::into)
}

pub(super) fn decode(value: &ControlValue) -> Result<ProviderBindingRecord> {
    if value.class() != ControlRecordClass::Identity || value.len() > LIMIT { return Err(BAD); }
    let mut input = Input(value.as_bytes());
    if input.take(8)? != MAGIC { return Err(BAD); }
    let binding_id = BindingId::from_bytes(input.array()?);
    let installation_id = InstallationId::from_bytes(input.array()?);
    let installation_incarnation_id = InstallationIncarnationId::from_bytes(input.array()?);
    let peer_role = match input.byte()? {
        0 => PeerRole::StandaloneCli, 1 => PeerRole::ClientAdapter, _ => return Err(BAD),
    };
    let peer_identity_digest = Blake3Digest32::from_bytes(input.array()?);
    let pairing_generation = NonZeroRevision::new(input.u64()?).map_err(|_| BAD)?;
    let count = input.length(MAX_SET_ITEMS)?;
    // A nonempty profile has a four-byte length and at least one UTF-8 byte.
    if count > input.0.len() / 5 { return Err(BAD); }
    let mut profiles = Vec::new();
    profiles.try_reserve_exact(count).map_err(|_| BAD)?;
    for _ in 0..count {
        let profile = ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?).map_err(|_| BAD)?;
        if profiles.last().is_some_and(|previous| previous >= &profile) { return Err(BAD); }
        profiles.push(profile);
    }
    let permitted_profile_ids = BoundedSet::from_items(profiles).map_err(|_| BAD)?;
    let disclosure_ceiling_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?).map_err(|_| BAD)?;
    let issued_at = input.timestamp()?;
    let expires_at = match input.byte()? {
        0 => None, 1 => Some(input.timestamp()?), _ => return Err(BAD),
    };
    let revocation_generation = NonZeroRevision::new(input.u64()?).map_err(|_| BAD)?;
    let status = match input.byte()? {
        0 => ProviderBindingStatus::Active,
        1 => ProviderBindingStatus::Revoked,
        2 => ProviderBindingStatus::Expired,
        _ => return Err(BAD),
    };
    if !input.0.is_empty() { return Err(BAD); }
    let record = ProviderBindingRecord { binding_id, installation_id, installation_incarnation_id,
        peer_role, peer_identity_digest, pairing_generation, permitted_profile_ids,
        disclosure_ceiling_ref, issued_at, expires_at, revocation_generation, status };
    record.validate()?;
    Ok(record)
}

fn append(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    if out.len().checked_add(bytes.len()).is_none_or(|end| end > LIMIT) { return Err(BAD); }
    out.try_reserve(bytes.len()).map_err(|_| BAD)?;
    out.extend_from_slice(bytes);
    Ok(())
}
fn text(out: &mut Vec<u8>, value: &str) -> Result<()> {
    append(out, &u32::try_from(value.len()).map_err(|_| BAD)?.to_be_bytes())?;
    append(out, value.as_bytes())
}

struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let value = self.0.get(..count).ok_or(BAD)?;
        self.0 = &self.0[count..];
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> { self.take(N)?.try_into().map_err(|_| BAD) }
    fn byte(&mut self) -> Result<u8> { Ok(self.array::<1>()?[0]) }
    fn u64(&mut self) -> Result<u64> { Ok(u64::from_be_bytes(self.array()?)) }
    fn length(&mut self, limit: usize) -> Result<usize> {
        let size = usize::try_from(u32::from_be_bytes(self.array()?)).map_err(|_| BAD)?;
        if size > limit { return Err(BAD); }
        Ok(size)
    }
    fn text(&mut self, limit: usize) -> Result<&'a str> {
        let count = self.length(limit)?;
        std::str::from_utf8(self.take(count)?).map_err(|_| BAD)
    }
    fn timestamp(&mut self) -> Result<UtcTimestamp> {
        UtcTimestamp::parse(std::str::from_utf8(self.take(27)?).map_err(|_| BAD)?).map_err(|_| BAD)
    }
}
