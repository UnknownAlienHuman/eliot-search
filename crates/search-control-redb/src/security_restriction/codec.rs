//! Versioned, canonical, finite restart-command bytes. No access decisions.

use search_contracts::{
    DataRootId, InstallationIncarnationId, MAX_OPAQUE_ID_BYTES, MAX_OPAQUE_REF_BYTES, OwnerEpoch,
};

use super::*;
use crate::policy_codec::ACCESS_POLICY_RECORD_LEN;

const MAGIC: &[u8; 8] = b"ELSECR01";
const VERSION: u32 = 1;

pub(super) fn encode(value: &SecurityRestrictionMutation) -> Result<Vec<u8>, ControlError> {
    value.validate()?;
    let mut out = Encoder(Vec::new());
    out.put(MAGIC)?;
    out.u32(VERSION)?;
    out.put(value.identity.installation_incarnation_id.as_bytes())?;
    out.put(value.identity.data_root_id.as_bytes())?;
    out.u64(value.identity.owner_epoch.get())?;
    out.put(value.identity.path_identity_digest.as_bytes())?;
    out.put(value.identity.schema_family_digest.as_bytes())?;
    out.u32(value.identity.schema_version)?;
    out.u64(value.expected_generation)?;
    out.put(value.command_digest.as_bytes())?;
    out.text(value.operation_id.as_str())?;
    out.flag(value.expected_policy.is_some())?;
    if let Some(policy) = &value.expected_policy { out.put(&encode_access_policy(policy))?; }
    out.flag(value.expected_state.is_some())?;
    if let Some(state) = &value.expected_state { out.state(state)?; }
    out.state(&value.replacement)?;
    out.count(value.dependents.len())?;
    for owner in &value.dependents { out.text(owner.as_str())?; }
    Ok(out.0)
}

pub(super) fn decode(bytes: &[u8]) -> Result<SecurityRestrictionMutation, ControlError> {
    if bytes.len() > MAX_RESTRICTION_RECORD_BYTES { return Err(ControlError::StoreCorrupt); }
    let mut input = Decoder { bytes, position: 0 };
    if input.take(8)? != MAGIC || input.u32()? != VERSION { return Err(ControlError::StoreCorrupt); }
    let identity = JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes(input.array()?),
        data_root_id: DataRootId::from_bytes(input.array()?),
        owner_epoch: OwnerEpoch::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?,
        path_identity_digest: Blake3Digest32::from_bytes(input.array()?),
        schema_family_digest: Blake3Digest32::from_bytes(input.array()?),
        schema_version: input.u32()?,
    };
    let expected_generation = input.u64()?;
    let command_digest = Blake3Digest32::from_bytes(input.array()?);
    let operation_id = OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let expected_policy = if input.flag()? { Some(input.policy()?) } else { None };
    let expected_state = if input.flag()? { Some(input.state()?) } else { None };
    let replacement = input.state()?;
    let count = input.count(MAX_RESTRICTION_DEPENDENTS)?;
    let mut dependents = BoundedSet::empty();
    let mut previous: Option<OpaqueId> = None;
    for _ in 0..count {
        let owner = OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
        if previous.as_ref().is_some_and(|old| old >= &owner) { return Err(ControlError::StoreCorrupt); }
        previous = Some(owner.clone());
        dependents.insert(owner).map_err(|_| ControlError::StoreCorrupt)?;
    }
    let value = SecurityRestrictionMutation {
        identity, expected_generation, command_digest, operation_id,
        expected_policy, expected_state, replacement, dependents,
    };
    // Require one representation, including any constructor's text normalization.
    // A valid prefix, reordered set, duplicate item or repaired flag is not valid.
    if input.position != bytes.len()
        || encode(&value).map_err(|_| ControlError::StoreCorrupt)?.as_slice() != bytes
    { return Err(ControlError::StoreCorrupt); }
    Ok(value)
}

struct Encoder(Vec<u8>);

impl Encoder {
    fn put(&mut self, bytes: &[u8]) -> Result<(), ControlError> {
        let length = self.0.len().checked_add(bytes.len()).ok_or(ControlError::BudgetExceeded)?;
        if length > MAX_RESTRICTION_RECORD_BYTES { return Err(ControlError::BudgetExceeded); }
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), ControlError> { self.put(&value.to_be_bytes()) }
    fn u64(&mut self, value: u64) -> Result<(), ControlError> { self.put(&value.to_be_bytes()) }
    fn flag(&mut self, value: bool) -> Result<(), ControlError> { self.put(&[u8::from(value)]) }
    fn count(&mut self, value: usize) -> Result<(), ControlError> {
        self.u32(u32::try_from(value).map_err(|_| ControlError::BudgetExceeded)?)
    }
    fn text(&mut self, value: &str) -> Result<(), ControlError> {
        self.count(value.len())?;
        self.put(value.as_bytes())
    }
    fn members(&mut self, members: &BoundedSet<SourceMembershipId, MAX_SET_ITEMS>) -> Result<(), ControlError> {
        self.count(members.len())?;
        for member in members { self.put(member.as_bytes())?; }
        Ok(())
    }
    fn state(&mut self, state: &SecurityPolicyState) -> Result<(), ControlError> {
        self.put(&encode_access_policy(&state.policy))?;
        self.text(state.security_domain_ref.as_str())?;
        self.put(state.snapshot_digest.as_bytes())?;
        self.flag(state.fail_closed)?;
        self.members(&state.denied_memberships)?;
        self.members(&state.purged_memberships)
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ControlError> {
        let end = self.position.checked_add(count).ok_or(ControlError::StoreCorrupt)?;
        let bytes = self.bytes.get(self.position..end).ok_or(ControlError::StoreCorrupt)?;
        self.position = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        self.take(N)?.try_into().map_err(|_| ControlError::StoreCorrupt)
    }
    fn u32(&mut self) -> Result<u32, ControlError> { self.array().map(u32::from_be_bytes) }
    fn u64(&mut self) -> Result<u64, ControlError> { self.array().map(u64::from_be_bytes) }
    fn flag(&mut self) -> Result<bool, ControlError> {
        match self.take(1)? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(ControlError::StoreCorrupt),
        }
    }
    fn count(&mut self, maximum: usize) -> Result<usize, ControlError> {
        let count = usize::try_from(self.u32()?).map_err(|_| ControlError::StoreCorrupt)?;
        if count > maximum { return Err(ControlError::StoreCorrupt); }
        Ok(count)
    }
    fn text(&mut self, maximum: usize) -> Result<&'a str, ControlError> {
        let count = self.count(maximum)?;
        std::str::from_utf8(self.take(count)?).map_err(|_| ControlError::StoreCorrupt)
    }
    fn policy(&mut self) -> Result<AccessPolicyRecord, ControlError> {
        decode_access_policy(self.take(ACCESS_POLICY_RECORD_LEN)?).map_err(|_| ControlError::StoreCorrupt)
    }
    fn members(&mut self) -> Result<BoundedSet<SourceMembershipId, MAX_SET_ITEMS>, ControlError> {
        let count = self.count(MAX_SET_ITEMS)?;
        // Check the entire advertised byte range before allocating any set nodes.
        let bytes = self.take(count.checked_mul(16).ok_or(ControlError::StoreCorrupt)?)?;
        let mut result = BoundedSet::empty();
        let mut previous = None;
        for bytes in bytes.chunks_exact(16) {
            let member = SourceMembershipId::from_bytes(bytes.try_into().map_err(|_| ControlError::StoreCorrupt)?);
            if previous.is_some_and(|old| old >= member) { return Err(ControlError::StoreCorrupt); }
            previous = Some(member);
            result.insert(member).map_err(|_| ControlError::StoreCorrupt)?;
        }
        Ok(result)
    }
    fn state(&mut self) -> Result<SecurityPolicyState, ControlError> {
        Ok(SecurityPolicyState {
            policy: self.policy()?,
            security_domain_ref: OpaqueRef::new(self.text(MAX_OPAQUE_REF_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?,
            snapshot_digest: Blake3Digest32::from_bytes(self.array()?),
            fail_closed: self.flag()?,
            denied_memberships: self.members()?,
            purged_memberships: self.members()?,
        })
    }
}
