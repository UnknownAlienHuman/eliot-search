//! Closed S9.5 collection schema: one immutable generation's exact contract.
//!
//! The schema owns no vendor type, no provider connection and no physical
//! collection string. Its identity is internally computed through the #237
//! canonical BLAKE3 helpers; a caller may never supply a schema identity.

use crate::{
    Blake3Digest32, CanonicalValue, CollectionGenerationId, ContractError, ContractErrorKind,
    DigestInputLimit, InstallationIncarnationId, MAX_QDRANT_EPOCH, MIN_QDRANT_EPOCH,
    ProjectionProfileSetId, blake3_canonical, parse_canonical_cbor, to_canonical_cbor,
};

use super::codec::{
    array, decode_array, decode_text, epoch, error, object, take_bool, take_text, take_u64,
};
use super::vectors::{MAX_NAMED_VECTORS, NamedVectorRequirement, VectorMode};

/// Frozen schema identity domain. The payload/index/vector tables are pinned by
/// this profile, so a changed table requires a new profile and generation.
const SCHEMA_DOMAIN: &str = "eliot/cbor/indexed-schema/v1";
/// Frozen value emitted inside the identity preimage. One generation's
/// compatibility cannot be reinterpreted by a later decoder.
const SCHEMA_VALUE: &str = "indexed-collection-schema/v1";
/// Frozen fixture/profile version bound into the identity preimage.
const SCHEMA_FIXTURE_VERSION: u64 = 1;

/// Compact ceiling for the closed schema preimage. The 20-field payload table
/// and 19-index plan dominate it; a dense vector list may widen it.
const SCHEMA_PREIMAGE_LIMIT: usize = 64 * 1024;

const FIELD_BINDING: &str = "binding";
const FIELD_MODE: &str = "vector_mode";
const FIELD_VECTORS: &str = "vector_requirements";
const FIELD_INVARIANTS: &str = "invariants";
const FIELD_EPOCH_MIN: &str = "epoch_min";
const FIELD_EPOCH_MAX: &str = "epoch_max";
const FIELD_PAYLOAD_PLAN: &str = "payload_plan";
const FIELD_INDEX_PLAN: &str = "index_plan";
const FIELD_VALUE: &str = "value";
const FIELD_FIXTURE_VERSION: &str = "fixture_version";
const FIELD_FIXTURE_DIGEST: &str = "fixture_table_digest";
const FIELD_CANONICAL_PROFILE: &str = "canonical_profile";

const FIELD_INSTALLATION: &str = "installation_incarnation_id";
const FIELD_GENERATION: &str = "collection_generation_id";
const FIELD_PROFILE: &str = "projection_profile_set_id";
const FIELD_NODES: &str = "nodes";
const FIELD_SHARDS: &str = "shards";
const FIELD_REPLICATION: &str = "replication_factor";
const FIELD_WRITE_CONSISTENCY: &str = "write_consistency_factor";
const FIELD_STRICT_MODE: &str = "strict_mode";
const FIELD_WAIT_FOR_MUTATIONS: &str = "wait_for_mutations";
const FIELD_STRONG_ORDERING: &str = "strong_ordering";

/// Strictly one baseline: every numeric invariant is `1`, every bool is `true`.
///
/// A weakened or incompatible variant is refused, so a later generation cannot
/// silently drop durability or ordering and keep the same schema identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionInvariants {
    pub nodes: u32,
    pub shards: u32,
    pub replication_factor: u32,
    pub write_consistency_factor: u32,
    pub strict_mode: bool,
    pub wait_for_mutations: bool,
    pub strong_ordering: bool,
}

impl CollectionInvariants {
    /// The single accepted baseline for the first enabled generation.
    pub const BASELINE: Self = Self {
        nodes: 1,
        shards: 1,
        replication_factor: 1,
        write_consistency_factor: 1,
        strict_mode: true,
        wait_for_mutations: true,
        strong_ordering: true,
    };

    #[must_use]
    pub const fn nodes(self) -> u32 {
        self.nodes
    }

    #[must_use]
    pub const fn shards(self) -> u32 {
        self.shards
    }

    #[must_use]
    pub const fn replication_factor(self) -> u32 {
        self.replication_factor
    }

    #[must_use]
    pub const fn write_consistency_factor(self) -> u32 {
        self.write_consistency_factor
    }

    #[must_use]
    pub const fn strict_mode(self) -> bool {
        self.strict_mode
    }

    #[must_use]
    pub const fn wait_for_mutations(self) -> bool {
        self.wait_for_mutations
    }

    #[must_use]
    pub const fn strong_ordering(self) -> bool {
        self.strong_ordering
    }

    fn to_canonical_value(self) -> Result<CanonicalValue, ContractError> {
        object([
            (FIELD_NODES, CanonicalValue::U64(u64::from(self.nodes))),
            (FIELD_SHARDS, CanonicalValue::U64(u64::from(self.shards))),
            (
                FIELD_REPLICATION,
                CanonicalValue::U64(u64::from(self.replication_factor)),
            ),
            (
                FIELD_WRITE_CONSISTENCY,
                CanonicalValue::U64(u64::from(self.write_consistency_factor)),
            ),
            (FIELD_STRICT_MODE, CanonicalValue::Bool(self.strict_mode)),
            (
                FIELD_WAIT_FOR_MUTATIONS,
                CanonicalValue::Bool(self.wait_for_mutations),
            ),
            (
                FIELD_STRONG_ORDERING,
                CanonicalValue::Bool(self.strong_ordering),
            ),
        ])
    }

    fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let mut fields = crate::ClosedCanonicalObject::from_value(value, FIELD_INVARIANTS)?;
        let parsed = Self {
            nodes: u32::try_from(take_u64(&mut fields, FIELD_NODES)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_NODES))?,
            shards: u32::try_from(take_u64(&mut fields, FIELD_SHARDS)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_SHARDS))?,
            replication_factor: u32::try_from(take_u64(&mut fields, FIELD_REPLICATION)?)
                .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_REPLICATION))?,
            write_consistency_factor: u32::try_from(take_u64(
                &mut fields,
                FIELD_WRITE_CONSISTENCY,
            )?)
            .map_err(|_| error(ContractErrorKind::MalformedPayload, FIELD_WRITE_CONSISTENCY))?,
            strict_mode: take_bool(&mut fields, FIELD_STRICT_MODE)?,
            wait_for_mutations: take_bool(&mut fields, FIELD_WAIT_FOR_MUTATIONS)?,
            strong_ordering: take_bool(&mut fields, FIELD_STRONG_ORDERING)?,
        };
        fields.finish()?;
        Ok(parsed)
    }

    const fn is_baseline(self) -> bool {
        self.nodes == Self::BASELINE.nodes
            && self.shards == Self::BASELINE.shards
            && self.replication_factor == Self::BASELINE.replication_factor
            && self.write_consistency_factor == Self::BASELINE.write_consistency_factor
            && self.strict_mode == Self::BASELINE.strict_mode
            && self.wait_for_mutations == Self::BASELINE.wait_for_mutations
            && self.strong_ordering == Self::BASELINE.strong_ordering
    }
}

/// Internally computed closed schema identity. No caller-supplied restore.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IndexedSchemaId(Blake3Digest32);

impl IndexedSchemaId {
    #[must_use]
    pub const fn as_digest(&self) -> &Blake3Digest32 {
        &self.0
    }
}

/// One immutable collection generation's vendor-neutral contract.
///
/// The binding, the ordered 20-field payload plan, the derived 19-index plan,
/// the vector requirements, the closed epoch domain, the canonical profile and
/// the frozen fixture version are all bound into one internally computed
/// identity. A caller cannot relabel a different collection as this one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    installation_incarnation_id: InstallationIncarnationId,
    collection_generation_id: CollectionGenerationId,
    projection_profile_set_id: ProjectionProfileSetId,
    mode: VectorMode,
    vectors: Vec<NamedVectorRequirement>,
    invariants: CollectionInvariants,
}

impl CollectionSchema {
    /// Builds the first enabled generation: sparse-only, baseline invariants.
    pub fn initial(
        installation: InstallationIncarnationId,
        generation: CollectionGenerationId,
        profile: ProjectionProfileSetId,
        vectors: &[NamedVectorRequirement],
    ) -> Result<Self, ContractError> {
        Self::new(
            installation,
            generation,
            profile,
            VectorMode::SparseOnly,
            vectors,
            CollectionInvariants::BASELINE,
        )
    }

    /// Builds one schema. The caller supplies only coordinates and requirements;
    /// the identity is computed, never provided.
    pub fn new(
        installation: InstallationIncarnationId,
        generation: CollectionGenerationId,
        profile: ProjectionProfileSetId,
        mode: VectorMode,
        vectors: &[NamedVectorRequirement],
        invariants: CollectionInvariants,
    ) -> Result<Self, ContractError> {
        validate_schema_shape(mode, vectors, invariants)?;
        let mut vectors = vectors.to_vec();
        vectors.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        let schema = Self {
            installation_incarnation_id: installation,
            collection_generation_id: generation,
            projection_profile_set_id: profile,
            mode,
            vectors,
            invariants,
        };
        schema.validate()?;
        Ok(schema)
    }

    /// Validates the exact tables and bounds before any identity is derived.
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_shape(self.mode, &self.vectors, self.invariants)
    }

    #[must_use]
    pub const fn installation_incarnation_id(&self) -> &InstallationIncarnationId {
        &self.installation_incarnation_id
    }

    #[must_use]
    pub const fn collection_generation_id(&self) -> &CollectionGenerationId {
        &self.collection_generation_id
    }

    #[must_use]
    pub const fn projection_profile_set_id(&self) -> &ProjectionProfileSetId {
        &self.projection_profile_set_id
    }

    #[must_use]
    pub const fn mode(&self) -> VectorMode {
        self.mode
    }

    #[must_use]
    pub fn vectors(&self) -> &[NamedVectorRequirement] {
        &self.vectors
    }

    #[must_use]
    pub const fn invariants(&self) -> CollectionInvariants {
        self.invariants
    }

    /// Computes the closed identity through the #237 canonical BLAKE3 profile.
    pub fn identity(&self) -> Result<IndexedSchemaId, ContractError> {
        let preimage = self.to_canonical_value()?;
        let domain = crate::CanonicalDigestDomain::parse(SCHEMA_DOMAIN)?;
        let digest = blake3_canonical(
            &domain,
            &preimage,
            DigestInputLimit::new(SCHEMA_PREIMAGE_LIMIT)?,
        )?;
        Ok(IndexedSchemaId(digest))
    }

    /// Decodes the exact fixed tables, recomputes the identity and only then
    /// compares the caller's expected digest. A mismatch is not a partial pass.
    pub fn restore(bytes: &[u8], expected: Blake3Digest32) -> Result<Self, ContractError> {
        let decoded = Self::from_canonical_cbor(bytes)?;
        let computed = decoded.identity()?;
        if *computed.as_digest() != expected {
            return Err(error(
                ContractErrorKind::DigestMismatch,
                FIELD_FIXTURE_DIGEST,
            ));
        }
        Ok(decoded)
    }

    /// Exact provider-provided readback: no extra, missing or wrong-typed
    /// index, and no vector or invariant substitution.
    pub fn validate_readback(
        &self,
        indexes: &[(String, crate::indexed::PayloadIndexKind)],
        vectors: &[NamedVectorRequirement],
        invariants: CollectionInvariants,
    ) -> Result<(), ContractError> {
        crate::indexed::validate_payload_indexes(indexes)?;
        if invariants != self.invariants {
            return Err(error(
                ContractErrorKind::ContradictoryState,
                FIELD_INVARIANTS,
            ));
        }
        if vectors.len() != self.vectors.len() {
            return Err(error(ContractErrorKind::MalformedPayload, FIELD_VECTORS));
        }
        for (position, vector) in vectors.iter().enumerate() {
            let expected = self
                .vectors
                .iter()
                .find(|expected| expected.name == vector.name)
                .ok_or_else(|| error(ContractErrorKind::UnknownField, FIELD_VECTORS))?;
            if vector.requirement != expected.requirement {
                return Err(error(ContractErrorKind::MalformedPayload, FIELD_VECTORS));
            }
            if vectors[..position]
                .iter()
                .any(|other| other.name == vector.name)
            {
                return Err(error(ContractErrorKind::Duplicate, FIELD_VECTORS));
            }
        }
        Ok(())
    }

    pub fn to_canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        self.validate()?;
        object([
            (FIELD_BINDING, self.binding_value()?),
            (FIELD_PAYLOAD_PLAN, crate::indexed::payload_plan_value()?),
            (FIELD_INDEX_PLAN, crate::indexed::index_plan_value()?),
            (FIELD_MODE, super::codec::text(self.mode.as_str())?),
            (FIELD_VECTORS, self.vectors_value()?),
            (FIELD_INVARIANTS, self.invariants.to_canonical_value()?),
            (
                FIELD_EPOCH_MIN,
                CanonicalValue::U64(MIN_QDRANT_EPOCH as u64),
            ),
            (
                FIELD_EPOCH_MAX,
                CanonicalValue::U64(MAX_QDRANT_EPOCH as u64),
            ),
            (FIELD_VALUE, super::codec::text(SCHEMA_VALUE)?),
            (
                FIELD_FIXTURE_VERSION,
                CanonicalValue::U64(SCHEMA_FIXTURE_VERSION),
            ),
            (
                FIELD_CANONICAL_PROFILE,
                super::codec::text(crate::CANONICAL_CBOR_PROFILE)?,
            ),
            (
                FIELD_FIXTURE_DIGEST,
                CanonicalValue::Bytes(crate::BoundedBytes::new(
                    fixture_table_digest()?.as_bytes().to_vec(),
                )?),
            ),
        ])
    }

    fn binding_value(&self) -> Result<CanonicalValue, ContractError> {
        object([
            (
                FIELD_INSTALLATION,
                super::codec::text(&self.installation_incarnation_id.to_string())?,
            ),
            (
                FIELD_GENERATION,
                super::codec::text(&self.collection_generation_id.to_string())?,
            ),
            (
                FIELD_PROFILE,
                super::codec::text(self.projection_profile_set_id.as_str())?,
            ),
        ])
    }

    fn vectors_value(&self) -> Result<CanonicalValue, ContractError> {
        array(
            self.vectors
                .iter()
                .map(NamedVectorRequirement::to_canonical_value)
                .collect::<Result<Vec<_>, ContractError>>()?,
        )
    }

    pub fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let mut fields = crate::ClosedCanonicalObject::from_value(value, FIELD_FIXTURE_DIGEST)?;
        let mut binding = crate::ClosedCanonicalObject::from_value(
            take_required_value(&mut fields, FIELD_BINDING)?,
            FIELD_BINDING,
        )?;
        let installation = decode_text_required(&mut binding, FIELD_INSTALLATION)?;
        let generation = decode_text_required(&mut binding, FIELD_GENERATION)?;
        let profile = decode_text_required(&mut binding, FIELD_PROFILE)?;
        binding.finish()?;
        let payload_plan = take_required_value(&mut fields, FIELD_PAYLOAD_PLAN)?;
        let index_plan = take_required_value(&mut fields, FIELD_INDEX_PLAN)?;
        let mode = VectorMode::parse(&take_text(&mut fields, FIELD_MODE)?)?;
        let raw_vectors = decode_array(
            take_required_value(&mut fields, FIELD_VECTORS)?,
            FIELD_VECTORS,
        )?;
        if raw_vectors.len() > MAX_NAMED_VECTORS as usize {
            return Err(error(ContractErrorKind::TooManyItems, FIELD_VECTORS));
        }
        let vectors = raw_vectors
            .into_iter()
            .map(NamedVectorRequirement::from_canonical_value)
            .collect::<Result<Vec<_>, ContractError>>()?;
        let invariants = CollectionInvariants::from_canonical_value(take_required_value(
            &mut fields,
            FIELD_INVARIANTS,
        )?)?;
        let epoch_min = epoch(
            take_required_value(&mut fields, FIELD_EPOCH_MIN)?,
            FIELD_EPOCH_MIN,
        )?;
        let epoch_max = epoch(
            take_required_value(&mut fields, FIELD_EPOCH_MAX)?,
            FIELD_EPOCH_MAX,
        )?;
        let value_name = take_text(&mut fields, FIELD_VALUE)?;
        let fixture_version = take_u64(&mut fields, FIELD_FIXTURE_VERSION)?;
        let canonical_profile = take_text(&mut fields, FIELD_CANONICAL_PROFILE)?;
        match take_required_value(&mut fields, FIELD_FIXTURE_DIGEST)? {
            CanonicalValue::Bytes(bytes)
                if bytes.as_slice() == fixture_table_digest()?.as_bytes() => {}
            _ => {
                return Err(error(
                    ContractErrorKind::DigestMismatch,
                    FIELD_FIXTURE_DIGEST,
                ));
            }
        }
        fields.finish()?;
        if epoch_min.get() != MIN_QDRANT_EPOCH || epoch_max.get() != MAX_QDRANT_EPOCH {
            return Err(error(ContractErrorKind::EpochOutOfRange, FIELD_EPOCH_MAX));
        }
        if value_name != SCHEMA_VALUE
            || fixture_version != SCHEMA_FIXTURE_VERSION
            || canonical_profile != crate::CANONICAL_CBOR_PROFILE
        {
            return Err(error(
                ContractErrorKind::UnsupportedVersion,
                FIELD_FIXTURE_VERSION,
            ));
        }
        if payload_plan != crate::indexed::payload_plan_value()?
            || index_plan != crate::indexed::index_plan_value()?
        {
            return Err(error(
                ContractErrorKind::MalformedPayload,
                FIELD_PAYLOAD_PLAN,
            ));
        }
        Self::new(
            InstallationIncarnationId::parse(&installation)?,
            CollectionGenerationId::parse(&generation)?,
            ProjectionProfileSetId::new(profile)?,
            mode,
            &vectors,
            invariants,
        )
    }

    pub fn to_canonical_cbor(&self) -> Result<crate::CanonicalBytes, ContractError> {
        to_canonical_cbor(&self.to_canonical_value()?)
    }

    pub fn from_canonical_cbor(bytes: &[u8]) -> Result<Self, ContractError> {
        if bytes.len() > SCHEMA_PREIMAGE_LIMIT {
            return Err(ContractError::oversize(
                "indexed_schema",
                SCHEMA_PREIMAGE_LIMIT,
                bytes.len(),
            ));
        }
        Self::from_canonical_value(parse_canonical_cbor(bytes)?)
    }
}

fn take_required_value(
    fields: &mut crate::ClosedCanonicalObject,
    field: &'static str,
) -> Result<CanonicalValue, ContractError> {
    fields
        .take_required(field)
        .map_err(|_| error(ContractErrorKind::MalformedPayload, field))
}

fn decode_text_required(
    fields: &mut crate::ClosedCanonicalObject,
    field: &'static str,
) -> Result<String, ContractError> {
    decode_text(take_required_value(fields, field)?, field)
}

/// The closed table digest, independently derived so the identity preimage and
/// the physical baseline tables cannot drift without a new profile.
fn fixture_table_digest() -> Result<Blake3Digest32, ContractError> {
    let table = object([
        (FIELD_PAYLOAD_PLAN, crate::indexed::payload_plan_value()?),
        (FIELD_INDEX_PLAN, crate::indexed::index_plan_value()?),
        (
            FIELD_EPOCH_MIN,
            CanonicalValue::U64(MIN_QDRANT_EPOCH as u64),
        ),
        (
            FIELD_EPOCH_MAX,
            CanonicalValue::U64(MAX_QDRANT_EPOCH as u64),
        ),
        (
            FIELD_FIXTURE_VERSION,
            CanonicalValue::U64(SCHEMA_FIXTURE_VERSION),
        ),
        (
            FIELD_CANONICAL_PROFILE,
            super::codec::text(crate::CANONICAL_CBOR_PROFILE)?,
        ),
    ])?;
    let domain = crate::CanonicalDigestDomain::parse("eliot/cbor/indexed-fixture-table/v1")?;
    blake3_canonical(
        &domain,
        &table,
        DigestInputLimit::new(SCHEMA_PREIMAGE_LIMIT)?,
    )
}

fn validate_schema_shape(
    mode: VectorMode,
    vectors: &[NamedVectorRequirement],
    invariants: CollectionInvariants,
) -> Result<(), ContractError> {
    if !invariants.is_baseline() {
        return Err(error(
            ContractErrorKind::ContradictoryState,
            FIELD_INVARIANTS,
        ));
    }
    crate::indexed::validate_vector_requirements(mode, vectors)
}
