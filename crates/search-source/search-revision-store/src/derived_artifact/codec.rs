//! Closed canonical descriptor codec; no body data is serialized here.
use super::{
    DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1 as Error,
    DerivedArtifactKindV1 as Kind, DerivedOwnerProfileV1, MAX_DERIVED_DESCRIPTOR_BYTES,
};
use search_contracts::{self as c, CanonicalValue as V};

pub(super) fn bytes(value: &[u8]) -> Result<V, Error> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| Error::BudgetExceeded)
}
pub(super) fn text(value: &str) -> Result<V, Error> {
    c::CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| Error::BudgetExceeded)
}
pub(super) fn array(values: Vec<V>) -> Result<V, Error> {
    c::BoundedList::new(values)
        .map(V::Array)
        .map_err(|_| Error::BudgetExceeded)
}
pub(super) fn encode(value: &V, maximum: usize) -> Result<Vec<u8>, Error> {
    let encoded = c::to_canonical_cbor(value)
        .map_err(|_| Error::BudgetExceeded)?
        .into_vec();
    if encoded.len() > maximum {
        return Err(Error::BudgetExceeded);
    }
    Ok(encoded)
}
pub(super) fn parse(encoded: &[u8], maximum: usize) -> Result<V, Error> {
    if encoded.is_empty() || encoded.len() > maximum {
        return Err(Error::BudgetExceeded);
    }
    c::parse_canonical_cbor(encoded).map_err(|_| Error::ProfileInvalid)
}
pub(super) fn hash_bytes(encoded: &[u8], maximum: usize) -> Result<c::Blake3Digest32, Error> {
    c::blake3_bounded_bytes(
        encoded,
        c::Blake3ByteBudget::new(maximum).map_err(|_| Error::BudgetExceeded)?,
    )
    .map_err(|_| Error::BudgetExceeded)
}
pub(super) fn tagged(domain: &str, value: &V, maximum: usize) -> Result<c::Blake3Digest32, Error> {
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(domain).map_err(|_| Error::ProfileInvalid)?,
        value,
        c::DigestInputLimit::new(maximum).map_err(|_| Error::BudgetExceeded)?,
    )
    .map_err(|_| Error::BudgetExceeded)
}
pub(super) fn digest(bytes: &[u8]) -> Result<c::Blake3Digest32, Error> {
    Ok(c::Blake3Digest32::from_bytes(
        bytes.try_into().map_err(|_| Error::ProfileInvalid)?,
    ))
}
pub(super) fn encode_descriptor(value: &Descriptor) -> Result<Vec<u8>, Error> {
    value.validate()?;
    let source = array(vec![
        bytes(value.source.source_namespace_id.as_bytes())?,
        bytes(value.source.source_id.as_bytes())?,
        bytes(value.source.revision_id.as_bytes())?,
        bytes(value.source.content_digest.as_bytes())?,
        V::U64(value.source.byte_length),
    ])?;
    let profile = array(vec![
        text(value.profile.id.as_str())?,
        V::U64(value.profile.revision.get()),
        bytes(value.profile.identity_digest.as_bytes())?,
    ])?;
    encode(
        &array(vec![
            text("eliot.derived-artifact-descriptor.v1")?,
            V::U64(1),
            text(value.kind.as_str())?,
            source,
            bytes(value.materialization.as_bytes())?,
            bytes(value.representation.as_bytes())?,
            profile,
            bytes(value.semantic_digest.as_bytes())?,
            bytes(value.encoded_digest.as_bytes())?,
            V::U64(value.encoded_bytes),
            bytes(
                &crate::residency_canonical_bytes(&value.domains)
                    .map_err(|_| Error::ResidencyMismatch)?,
            )?,
            text(value.operation.as_str())?,
        ])?,
        MAX_DERIVED_DESCRIPTOR_BYTES,
    )
}
pub(super) fn decode_descriptor(encoded: &[u8]) -> Result<Descriptor, Error> {
    let V::Array(fields) = parse(encoded, MAX_DERIVED_DESCRIPTOR_BYTES)? else {
        return Err(Error::ProfileInvalid);
    };
    let [
        V::Text(marker),
        V::U64(1),
        V::Text(kind),
        V::Array(source),
        V::Bytes(mat),
        V::Bytes(rep),
        V::Array(profile),
        V::Bytes(semantic),
        V::Bytes(content),
        V::U64(length),
        V::Bytes(residency),
        V::Text(operation),
    ] = fields.as_slice()
    else {
        return Err(Error::ProfileInvalid);
    };
    if marker.as_str() != "eliot.derived-artifact-descriptor.v1" {
        return Err(Error::ProfileInvalid);
    }
    let [
        V::Bytes(ns),
        V::Bytes(source),
        V::Bytes(revision),
        V::Bytes(source_digest),
        V::U64(source_bytes),
    ] = source.as_slice()
    else {
        return Err(Error::ProfileInvalid);
    };
    let [
        V::Text(profile),
        V::U64(profile_revision),
        V::Bytes(profile_digest),
    ] = profile.as_slice()
    else {
        return Err(Error::ProfileInvalid);
    };
    let value = Descriptor {
        kind: Kind::parse(kind.as_str())?,
        source: c::SourceRevisionRef {
            source_namespace_id: c::SourceNamespaceId::from_bytes(uuid(ns.as_slice())?),
            source_id: c::SourceId::from_bytes(uuid(source.as_slice())?),
            revision_id: c::SourceRevisionId::from_bytes(uuid(revision.as_slice())?),
            content_digest: digest(source_digest.as_slice())?,
            byte_length: *source_bytes,
        },
        materialization: c::MaterializationId::from_bytes(uuid(mat.as_slice())?),
        representation: c::RepresentationId::from_bytes(uuid(rep.as_slice())?),
        profile: DerivedOwnerProfileV1 {
            id: c::ProfileId::new(profile.as_str()).map_err(|_| Error::ProfileInvalid)?,
            revision: c::NonZeroRevision::new(*profile_revision)
                .map_err(|_| Error::ProfileInvalid)?,
            identity_digest: digest(profile_digest.as_slice())?,
        },
        semantic_digest: digest(semantic.as_slice())?,
        encoded_digest: digest(content.as_slice())?,
        encoded_bytes: *length,
        domains: crate::decode_residency_canonical(residency.as_slice())
            .map_err(|_| Error::ResidencyMismatch)?,
        operation: c::OpaqueId::new(operation.as_str()).map_err(|_| Error::ProfileInvalid)?,
    };
    value.validate()?;
    if value.encode()? != encoded {
        return Err(Error::Mismatch);
    }
    Ok(value)
}
fn uuid(encoded: &[u8]) -> Result<[u8; 16], Error> {
    encoded.try_into().map_err(|_| Error::IdentityInvalid)
}
