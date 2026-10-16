//! One bounded four-object reference graph, independent of source-history mutation.
use super::{
    DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1 as Error,
    DerivedArtifactKindV1 as Kind, DerivedArtifactObjectRefV1 as ObjectRef,
    DerivedArtifactPayloadCandidateV1 as Candidate, DerivedOwnerProfileV1, codec as wire,
};
use search_contracts::{self as c, CanonicalValue as V};

/// Complete encoded set-manifest ceiling, including its four full references.
pub const MAX_DERIVED_SET_BYTES: usize = 4 * (super::MAX_DERIVED_DESCRIPTOR_BYTES + 512) + 256;
const KINDS: [Kind; 4] = [
    Kind::RepresentationBody,
    Kind::MaterializationMetadata,
    Kind::UnitManifest,
    Kind::ProjectionManifest,
];

/// Immutable four-object graph metadata.
///
/// A decoded/caller-built set grants no durability,
/// policy, retention or reachability receipt. I/O reopens every member before
/// publishing or returning a verified set observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedArtifactSetDescriptorV1 {
    members: [ObjectRef; 4],
}
impl DerivedArtifactSetDescriptorV1 {
    /// Bind exactly four objects in closed kind order, with identical genuine
    /// source/materialization/representation identities and complete residency.
    /// Root normally supplies references from C07's actual I/O receipts.
    pub fn new(members: [ObjectRef; 4]) -> Result<Self, Error> {
        let first = members[0].descriptor();
        for (member, kind) in members.iter().zip(KINDS) {
            let descriptor = member.descriptor();
            if descriptor.kind() != kind
                || descriptor.source() != first.source()
                || descriptor.materialization_id() != first.materialization_id()
                || descriptor.representation_id() != first.representation_id()
            {
                return Err(Error::Mismatch);
            }
            first.require_same_residency(descriptor)?;
        }
        if members[1].descriptor().owner_profile() != first.owner_profile() {
            return Err(Error::Mismatch);
        }
        let value = Self { members };
        value.encode()?;
        Ok(value)
    }
    /// Exact four references. They are metadata, not standalone durable receipts.
    pub const fn members(&self) -> &[ObjectRef; 4] {
        &self.members
    }
    fn value(&self) -> Result<V, Error> {
        wire::array(vec![
            wire::text("eliot.derived-artifact-set.v1")?,
            V::U64(1),
            wire::array(
                self.members
                    .iter()
                    .map(|member| wire::bytes(&member.encode()?))
                    .collect::<Result<Vec<_>, Error>>()?,
            )?,
        ])
    }
    /// Canonical body-free set manifest, with full exact references in kind order.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        wire::encode(&self.value()?, MAX_DERIVED_SET_BYTES)
    }
    /// Strict closed decode; no file existence or current reachability is inferred.
    pub fn decode(encoded: &[u8]) -> Result<Self, Error> {
        let V::Array(fields) = wire::parse(encoded, MAX_DERIVED_SET_BYTES)? else {
            return Err(Error::ProfileInvalid);
        };
        let [V::Text(profile), V::U64(1), V::Array(members)] = fields.as_slice() else {
            return Err(Error::ProfileInvalid);
        };
        if profile.as_str() != "eliot.derived-artifact-set.v1" || members.len() != 4 {
            return Err(Error::ProfileInvalid);
        }
        let mut decoded = Vec::with_capacity(4);
        for member in members.as_slice() {
            let V::Bytes(encoded) = member else {
                return Err(Error::ProfileInvalid);
            };
            decoded.push(ObjectRef::decode(encoded.as_slice())?);
        }
        let value = Self::new(decoded.try_into().map_err(|_| Error::ProfileInvalid)?)?;
        if value.encode()? != encoded {
            return Err(Error::Mismatch);
        }
        Ok(value)
    }
    /// C07-owned semantic commitment over the complete canonical reference graph.
    pub fn semantic_digest(&self) -> Result<c::Blake3Digest32, Error> {
        wire::tagged(
            "eliot/cbor/derived-artifact-set/v1",
            &self.value()?,
            MAX_DERIVED_SET_BYTES + 128,
        )
    }
    fn profile() -> Result<DerivedOwnerProfileV1, Error> {
        let schema = wire::array(vec![
            wire::text("eliot.derived-artifact-set.v1")?,
            V::U64(1),
            wire::array(
                KINDS
                    .iter()
                    .map(|kind| wire::text(kind.as_str()))
                    .collect::<Result<Vec<_>, _>>()?,
            )?,
        ])?;
        Ok(DerivedOwnerProfileV1 {
            id: c::ProfileId::new("eliot.derived-artifact-set.v1")
                .map_err(|_| Error::ProfileInvalid)?,
            revision: c::NonZeroRevision::new(1).map_err(|_| Error::ProfileInvalid)?,
            identity_digest: wire::tagged(
                "eliot/cbor/derived-artifact-set-profile/v1",
                &schema,
                1024,
            )?,
        })
    }
    /// Prepare an unqualified metadata candidate from this real C07 set codec.
    /// The I/O path still validates scope and reopens every member before effects.
    pub fn candidate(&self, operation: c::OpaqueId) -> Result<Candidate, Error> {
        let bytes = self.encode()?;
        let first = self.members[0].descriptor();
        let descriptor = Descriptor::from_encoded(
            Kind::SetManifest,
            *first.source(),
            first.materialization_id(),
            first.representation_id(),
            Self::profile()?,
            self.semantic_digest()?,
            first.domains().clone(),
            operation,
            &bytes,
            MAX_DERIVED_SET_BYTES,
        )?;
        Candidate::metadata(descriptor, bytes, MAX_DERIVED_SET_BYTES)
    }
    pub(super) fn verify_descriptor(&self, descriptor: &Descriptor) -> Result<(), Error> {
        let first = self.members[0].descriptor();
        if descriptor.kind() != Kind::SetManifest
            || descriptor.source() != first.source()
            || descriptor.materialization_id() != first.materialization_id()
            || descriptor.representation_id() != first.representation_id()
            || descriptor.owner_profile() != &Self::profile()?
            || descriptor.owner_semantic_digest() != self.semantic_digest()?
        {
            return Err(Error::Mismatch);
        }
        descriptor.require_same_residency(first)
    }
}
