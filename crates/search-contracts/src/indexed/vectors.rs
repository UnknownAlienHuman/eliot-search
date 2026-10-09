//! S9.5/S10.3 provider-neutral vector requirements and vector values.
//!
//! This module owns the closed vector vocabulary shared by projection
//! planning, publication/query validation and the Qdrant bridge. It carries
//! no vendor type, no physical collection string and no second canonical
//! encoder: every canonical representation is assembled through the
//! parent-owned indexed codec helpers over the accepted canonical value
//! vocabulary.

use crate::ContractError;
use crate::ContractErrorKind;
use crate::canonical::{BoundedName, CanonicalValue, ClosedCanonicalObject};
use crate::indexed::codec;

/// Closed named-vector mode for one collection generation.
///
/// The first enabled collection generation must explicitly declare whether
/// it is sparse-only or also supports dense vectors, so a planner cannot plan
/// dense vectors while a bridge silently rejects them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorMode {
    /// Only sparse named vectors are admitted.
    SparseOnly,
    /// Sparse named vectors plus at least one dense named vector that the
    /// schema actually requires. Dense presence is mandatory in this mode, so
    /// a declared `SparseWithDense` generation carrying no dense requirement is
    /// rejected instead of being silently degraded to sparse-only.
    SparseWithDense,
}

impl VectorMode {
    /// Every mode, in closed order.
    pub const ALL: [Self; 2] = [Self::SparseOnly, Self::SparseWithDense];

    /// Exact stable wire string for this mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SparseOnly => "sparse_only",
            Self::SparseWithDense => "sparse_with_dense",
        }
    }

    /// Closed parse. Unknown text fails closed.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` carrying `InvalidTaggedVariant` for any text
    /// outside the closed set.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        match value {
            "sparse_only" => Ok(Self::SparseOnly),
            "sparse_with_dense" => Ok(Self::SparseWithDense),
            _ => Err(ContractError::invalid_variant("vector_mode")),
        }
    }
}

/// Closed distance/profile identity for a dense named vector.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DenseDistance {
    /// Cosine distance.
    Cosine,
    /// Dot product.
    Dot,
    /// Euclidean distance.
    Euclid,
    /// Manhattan distance.
    Manhattan,
}

impl DenseDistance {
    /// Every distance, in closed order.
    pub const ALL: [Self; 4] = [Self::Cosine, Self::Dot, Self::Euclid, Self::Manhattan];

    /// Exact stable wire string for this distance.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::Dot => "dot",
            Self::Euclid => "euclid",
            Self::Manhattan => "manhattan",
        }
    }

    /// Closed parse. Unknown text fails closed.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` carrying `InvalidTaggedVariant` for any text
    /// outside the closed set.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        match value {
            "cosine" => Ok(Self::Cosine),
            "dot" => Ok(Self::Dot),
            "euclid" => Ok(Self::Euclid),
            "manhattan" => Ok(Self::Manhattan),
            _ => Err(ContractError::invalid_variant("dense_distance")),
        }
    }
}

/// Maximum number of named vectors admitted for one point.
pub const MAX_NAMED_VECTORS: u32 = 16;

/// Maximum number of stored values admitted for one point across all named
/// vectors.
pub const MAX_STORED_VECTOR_VALUES: u32 = 65_536;

/// Maximum accepted dense width.
pub const MAX_DENSE_DIMENSIONS: u32 = 65_536;

/// Maximum named-vector byte length.
pub const MAX_VECTOR_NAME_BYTES: usize = 128;

/// Closed named-vector identifier.
///
/// The byte ceiling and the identifier grammar are separate constraints and
/// both are checked against the borrowed input before any owned String is
/// materialised, so arbitrarily long caller text never allocates.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorName(BoundedName);

/// Checks the closed identifier grammar against borrowed bytes.
fn grammar_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    let Some(first) = bytes.first().copied() else {
        return false;
    };
    let Some(last) = bytes.last().copied() else {
        return false;
    };
    first.is_ascii_lowercase()
        && last.is_ascii_alphanumeric()
        && !value.contains("--")
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'-' | b'_')
        })
}

impl VectorName {
    /// Validates one bounded closed ASCII identifier.
    ///
    /// A non-empty lowercase ASCII name that starts with a letter and ends
    /// with a letter or digit, with internal digits, underscores and single hyphens. The
    /// byte ceiling and the grammar are checked on the borrowed input before
    /// any owned String exists.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the name is empty, exceeds
    /// `MAX_VECTOR_NAME_BYTES`, or carries text outside the closed grammar.
    pub fn new(value: &str) -> Result<Self, ContractError> {
        if value.is_empty() {
            return Err(ContractError::new(ContractErrorKind::Empty, "vector_name"));
        }
        if value.len() > MAX_VECTOR_NAME_BYTES {
            return Err(ContractError::oversize(
                "vector_name",
                MAX_VECTOR_NAME_BYTES,
                value.len(),
            ));
        }
        if !grammar_ok(value) {
            return Err(ContractError::new(
                ContractErrorKind::InvalidCharacter,
                "vector_name",
            ));
        }
        BoundedName::new(value.to_owned()).map(Self)
    }

    /// Exact identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl core::fmt::Display for VectorName {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Closed requirement for one named vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorRequirement {
    /// Sparse requirement. The index ceiling is mandatory and nonzero.
    Sparse {
        /// Exclusive upper bound on every stored sparse index.
        index_ceiling: u32,
        /// Whether the IDF modifier is part of scoring identity. Mandatory,
        /// because a missing modifier changes the scoring population.
        idf_enabled: bool,
    },
    /// Dense requirement. The width is mandatory and nonzero.
    Dense {
        /// Exact stored dense width.
        dimensions: u32,
        /// Accepted distance/profile identity.
        distance: DenseDistance,
    },
}

impl VectorRequirement {
    /// Closed representation tag shared by both variants.
    fn kind_str(self) -> &'static str {
        match self {
            Self::Sparse { .. } => "sparse",
            Self::Dense { .. } => "dense",
        }
    }

    /// Smallest number of stored values this requirement can ever carry.
    ///
    /// A dense requirement always stores exactly its width. A sparse
    /// requirement stores at least one value, because an empty sparse vector
    /// carries no index and is rejected by `VectorValue` validation.
    fn minimum_stored_values(self) -> u64 {
        match self {
            Self::Sparse { .. } => 1,
            Self::Dense { dimensions, .. } => u64::from(dimensions),
        }
    }

    /// Validates one requirement in isolation.
    ///
    /// A sparse ceiling must be nonzero and sparse IDF is mandatory, because
    /// a missing modifier changes the scoring population. A dense width must
    /// lie in the closed range 1..=`MAX_DENSE_DIMENSIONS`.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when a bound or the mandatory sparse IDF flag
    /// is violated.
    pub fn validate(&self) -> Result<(), ContractError> {
        match *self {
            Self::Sparse {
                index_ceiling,
                idf_enabled,
            } => {
                if index_ceiling == 0 {
                    return Err(ContractError::new(
                        ContractErrorKind::ZeroNotAllowed,
                        "index_ceiling",
                    ));
                }
                if !idf_enabled {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "idf_enabled",
                    ));
                }
                Ok(())
            }
            Self::Dense { dimensions, .. } => {
                if dimensions == 0 || dimensions > MAX_DENSE_DIMENSIONS {
                    return Err(ContractError::bounded(
                        ContractErrorKind::InvalidRange,
                        "dimensions",
                        u64::from(MAX_DENSE_DIMENSIONS),
                        u64::from(dimensions),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Exact canonical representation of one requirement.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the requirement fails validation.
    pub fn to_canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        self.validate()?;
        let kind = codec::text(self.kind_str())?;
        let fields: Vec<(&'static str, CanonicalValue)> = match *self {
            Self::Sparse {
                index_ceiling,
                idf_enabled,
            } => vec![
                ("kind", kind),
                (
                    "index_ceiling",
                    CanonicalValue::U64(u64::from(index_ceiling)),
                ),
                ("idf_enabled", CanonicalValue::Bool(idf_enabled)),
            ],
            Self::Dense {
                dimensions,
                distance,
            } => vec![
                ("kind", kind),
                ("dimensions", CanonicalValue::U64(u64::from(dimensions))),
                ("distance", codec::text(distance.as_str())?),
            ],
        };
        codec::object(fields)
    }

    /// Exact decode of one requirement.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the value is not an object, when a field
    /// is missing or unknown, when the kind tag is outside the closed set, or
    /// when the decoded requirement fails validation.
    pub fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let mut object = ClosedCanonicalObject::from_value(value, "vector_requirement")?;
        let kind = codec::take_text(&mut object, "kind")?;
        let requirement = match kind.as_str() {
            "sparse" => Self::Sparse {
                index_ceiling: take_u32_field(&mut object, "index_ceiling")?,
                idf_enabled: codec::take_bool(&mut object, "idf_enabled")?,
            },
            "dense" => Self::Dense {
                dimensions: take_u32_field(&mut object, "dimensions")?,
                distance: take_distance(&mut object, "distance")?,
            },
            _ => return Err(ContractError::invalid_variant("vector_requirement.kind")),
        };
        object.finish()?;
        requirement.validate()?;
        Ok(requirement)
    }
}

/// One named requirement: an exact identifier plus its closed requirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedVectorRequirement {
    /// Exact named-vector identifier.
    pub name: VectorName,
    /// Exact closed requirement.
    pub requirement: VectorRequirement,
}

impl NamedVectorRequirement {
    /// Builds one named requirement after validating the inner requirement.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the inner requirement is invalid. The
    /// caller-supplied name has already been validated by `VectorName`.
    pub fn new(name: VectorName, requirement: VectorRequirement) -> Result<Self, ContractError> {
        requirement.validate()?;
        Ok(Self { name, requirement })
    }

    /// Exact named-vector identifier.
    #[must_use]
    pub fn name(&self) -> &VectorName {
        &self.name
    }

    /// Exact closed requirement.
    #[must_use]
    pub fn requirement(&self) -> VectorRequirement {
        self.requirement
    }

    /// Exact canonical representation of one named requirement.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the inner requirement is invalid.
    pub fn to_canonical_value(&self) -> Result<CanonicalValue, ContractError> {
        let fields = vec![
            ("name", codec::text(self.name.as_str())?),
            ("requirement", self.requirement.to_canonical_value()?),
        ];
        codec::object(fields)
    }

    /// Exact decode of one named requirement.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` for a non-object value, a missing or unknown
    /// field, an invalid inner requirement, or an invalid name.
    pub fn from_canonical_value(value: CanonicalValue) -> Result<Self, ContractError> {
        let mut object = ClosedCanonicalObject::from_value(value, "named_vector_requirement")?;
        let name = codec::take_text(&mut object, "name")?;
        let requirement = VectorRequirement::from_canonical_value(
            object
                .take_optional("requirement")?
                .ok_or_else(|| ContractError::malformed("named_vector_requirement.requirement"))?,
        )?;
        object.finish()?;
        Self::new(VectorName::new(&name)?, requirement)
    }
}

/// Closed representation of a prepared vector value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorValueKind {
    /// Dense stored values.
    Dense,
    /// Sparse stored values with strictly increasing indices.
    Sparse,
}

/// Exact dense or sparse values produced by an encoder contract.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorValue {
    /// Dense finite values, one per stored dimension.
    Dense(Vec<f32>),
    /// Sparse finite values with strictly increasing indices below the
    /// declared ceiling.
    Sparse {
        /// Strictly increasing stored indices.
        indices: Vec<u32>,
        /// Finite values corresponding one-to-one with indices.
        values: Vec<f32>,
    },
}

impl VectorValue {
    /// Number of stored values carried by this representation.
    #[must_use]
    pub fn stored_values(&self) -> usize {
        match self {
            Self::Dense(values) | Self::Sparse { values, .. } => values.len(),
        }
    }

    /// Actual representation, independent of the declared requirement.
    #[must_use]
    pub const fn kind(&self) -> VectorValueKind {
        match self {
            Self::Dense(_) => VectorValueKind::Dense,
            Self::Sparse { .. } => VectorValueKind::Sparse,
        }
    }

    /// Validates exact shape, finiteness and order against one requirement.
    ///
    /// This check covers the per-vector ceiling only. The caller owns the
    /// per-point aggregate total and must enforce a checked running sum of
    /// `stored_values` across every named vector of one point against
    /// `MAX_STORED_VECTOR_VALUES` before any value iteration.
    ///
    /// # Errors
    ///
    /// Returns a `ContractError` when the representation disagrees with the
    /// requirement kind, when a value is non-finite, when sparse indices are
    /// not strictly increasing or reach the ceiling, or when the dense width
    /// does not match exactly.
    pub fn validate(&self, requirement: &VectorRequirement) -> Result<(), ContractError> {
        requirement.validate()?;
        if self.stored_values() > MAX_STORED_VECTOR_VALUES as usize {
            return Err(ContractError::bounded(
                ContractErrorKind::TooManyItems,
                "stored_vector_values",
                u64::from(MAX_STORED_VECTOR_VALUES),
                self.stored_values() as u64,
            ));
        }
        match (self, *requirement) {
            (Self::Dense(values), VectorRequirement::Dense { dimensions, .. }) => {
                let width = u32::try_from(values.len()).map_err(|_| {
                    ContractError::bounded(
                        ContractErrorKind::TooManyItems,
                        "dimensions",
                        u64::from(u32::MAX),
                        values.len() as u64,
                    )
                })?;
                if width != dimensions || values.iter().any(|value| !value.is_finite()) {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "dimensions",
                    ));
                }
                Ok(())
            }
            (Self::Sparse { indices, values }, VectorRequirement::Sparse { index_ceiling, .. }) => {
                if indices.len() != values.len() || indices.is_empty() {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "sparse_vector",
                    ));
                }
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(ContractError::new(
                        ContractErrorKind::InvalidRange,
                        "sparse_values",
                    ));
                }
                let mut previous = None;
                for index in indices {
                    if *index >= index_ceiling {
                        return Err(ContractError::bounded(
                            ContractErrorKind::InvalidRange,
                            "index_ceiling",
                            u64::from(index_ceiling),
                            u64::from(*index),
                        ));
                    }
                    if previous.is_some_and(|prior| prior >= *index) {
                        return Err(ContractError::new(
                            ContractErrorKind::Duplicate,
                            "sparse_indices",
                        ));
                    }
                    previous = Some(*index);
                }
                Ok(())
            }
            _ => Err(ContractError::new(
                ContractErrorKind::FamilyMismatch,
                "vector_requirement",
            )),
        }
    }
}

/// Decodes one bounded unsigned integer field from an open canonical object.
fn take_u32_field(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<u32, ContractError> {
    let raw = codec::take_u64(object, field)?;
    u32::try_from(raw).map_err(|_| {
        ContractError::bounded(
            ContractErrorKind::InvalidRange,
            field,
            u64::from(u32::MAX),
            raw,
        )
    })
}

/// Decodes one closed distance field from an open canonical object.
fn take_distance(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<DenseDistance, ContractError> {
    let raw = codec::take_text(object, field)?;
    DenseDistance::parse(&raw)
}

/// Validates one complete named-vector set against a declared mode.
///
/// The set must be non-empty, unique, and bounded by `MAX_NAMED_VECTORS`. It
/// must contain at least one sparse requirement. The declared mode must match
/// the actual dense presence: `SparseOnly` admits no dense requirement, and
/// `SparseWithDense` must carry at least one dense requirement.
///
/// The minimum required stored values across the whole set (the sum of exact
/// dense widths plus one value per sparse requirement) must also fit inside
/// `MAX_STORED_VECTOR_VALUES`, so a schema that no point could ever satisfy is
/// rejected here rather than at publication time.
///
/// # Errors
///
/// Returns a `ContractError` for an empty, duplicate, oversized, mode-disagreeing
/// or unsatisfiable set.
pub fn validate_vector_requirements(
    mode: VectorMode,
    requirements: &[NamedVectorRequirement],
) -> Result<(), ContractError> {
    if requirements.is_empty() {
        return Err(ContractError::new(
            ContractErrorKind::Empty,
            "named_vectors",
        ));
    }
    let count = u32::try_from(requirements.len()).map_err(|_| {
        ContractError::bounded(
            ContractErrorKind::TooManyItems,
            "named_vectors",
            u64::from(MAX_NAMED_VECTORS),
            requirements.len() as u64,
        )
    })?;
    if count > MAX_NAMED_VECTORS {
        return Err(ContractError::bounded(
            ContractErrorKind::TooManyItems,
            "named_vectors",
            u64::from(MAX_NAMED_VECTORS),
            u64::from(count),
        ));
    }

    // Names are short and capped, so the duplicate check stays cheap and is
    // performed before any owned growth of a working set.
    let mut names: Vec<&str> = Vec::new();
    let mut sparse_seen = 0_u32;
    let mut dense_seen = 0_u32;
    let mut minimum_values: u64 = 0;
    for entry in requirements {
        entry.requirement().validate()?;
        let name = entry.name().as_str();
        if names.contains(&name) {
            return Err(ContractError::new(
                ContractErrorKind::Duplicate,
                "vector_name",
            ));
        }
        names.push(name);
        minimum_values = minimum_values
            .checked_add(entry.requirement().minimum_stored_values())
            .ok_or_else(|| {
                ContractError::bounded(
                    ContractErrorKind::TooManyItems,
                    "named_vectors",
                    u64::from(MAX_STORED_VECTOR_VALUES),
                    u64::MAX,
                )
            })?;
        match entry.requirement() {
            VectorRequirement::Sparse { .. } => sparse_seen = sparse_seen.saturating_add(1),
            VectorRequirement::Dense { .. } => dense_seen = dense_seen.saturating_add(1),
        }
    }

    if minimum_values > u64::from(MAX_STORED_VECTOR_VALUES) {
        return Err(ContractError::bounded(
            ContractErrorKind::TooManyItems,
            "stored_vector_values",
            u64::from(MAX_STORED_VECTOR_VALUES),
            minimum_values,
        ));
    }
    if sparse_seen == 0 {
        return Err(ContractError::new(
            ContractErrorKind::Empty,
            "sparse_vectors",
        ));
    }
    match mode {
        VectorMode::SparseOnly if dense_seen > 0 => Err(ContractError::new(
            ContractErrorKind::InvalidTaggedVariant,
            "vector_mode",
        )),
        VectorMode::SparseWithDense if dense_seen == 0 => Err(ContractError::new(
            ContractErrorKind::InvalidTaggedVariant,
            "vector_mode",
        )),
        _ => Ok(()),
    }
}

/// Holds the mode declared for the first enabled collection generation.
///
/// The initial enabled decision is `SparseOnly`. Dense requirements describe
/// schema candidates only and do not by themselves enable a dense generation.
pub const INITIAL_VECTOR_MODE: VectorMode = VectorMode::SparseOnly;

#[cfg(test)]
mod tests {
    use super::*;

    fn sparse(ceiling: u32) -> NamedVectorRequirement {
        NamedVectorRequirement::new(
            VectorName::new("unit-sparse").expect("valid name"),
            VectorRequirement::Sparse {
                index_ceiling: ceiling,
                idf_enabled: true,
            },
        )
        .expect("valid requirement")
    }

    fn dense(dimensions: u32) -> NamedVectorRequirement {
        NamedVectorRequirement::new(
            VectorName::new("unit-dense").expect("valid name"),
            VectorRequirement::Dense {
                dimensions,
                distance: DenseDistance::Cosine,
            },
        )
        .expect("valid requirement")
    }

    #[test]
    fn sparse_only_requires_sparse_and_rejects_any_dense() {
        assert!(validate_vector_requirements(VectorMode::SparseOnly, &[sparse(1024)]).is_ok());
        assert!(
            validate_vector_requirements(VectorMode::SparseOnly, &[sparse(1024), dense(768)])
                .is_err()
        );
        assert!(validate_vector_requirements(VectorMode::SparseOnly, &[dense(768)]).is_err());
    }

    #[test]
    fn sparse_with_dense_requires_actual_dense_presence() {
        assert!(
            validate_vector_requirements(VectorMode::SparseWithDense, &[sparse(1024), dense(768)])
                .is_ok()
        );
        assert!(
            validate_vector_requirements(VectorMode::SparseWithDense, &[sparse(1024)]).is_err()
        );
        assert!(validate_vector_requirements(VectorMode::SparseWithDense, &[dense(768)]).is_err());
    }

    #[test]
    fn duplicate_names_are_rejected() {
        assert!(
            validate_vector_requirements(VectorMode::SparseOnly, &[sparse(8), sparse(16)]).is_err()
        );
    }

    #[test]
    fn oversize_named_vector_set_is_rejected() {
        let mut many = Vec::new();
        for index in 0..=MAX_NAMED_VECTORS {
            let name = VectorName::new(&format!("vector-{index}")).expect("valid name");
            many.push(
                NamedVectorRequirement::new(
                    name,
                    VectorRequirement::Sparse {
                        index_ceiling: 8,
                        idf_enabled: true,
                    },
                )
                .expect("valid requirement"),
            );
        }
        assert!(validate_vector_requirements(VectorMode::SparseOnly, &many).is_err());
    }

    #[test]
    fn unsatisfiable_schema_minimum_is_rejected() {
        let set = [sparse(8), dense(MAX_DENSE_DIMENSIONS)];
        assert_eq!(
            validate_vector_requirements(VectorMode::SparseWithDense, &set)
                .expect_err("one required sparse value plus maximum dense width exceeds aggregate")
                .field(),
            "stored_vector_values"
        );
    }

    #[test]
    fn satisfiable_dense_and_sparse_mix_is_accepted() {
        assert!(
            validate_vector_requirements(VectorMode::SparseWithDense, &[sparse(2048), dense(768)])
                .is_ok()
        );
    }

    #[test]
    fn sparse_ceiling_is_nonzero_and_idf_is_mandatory() {
        assert!(
            VectorRequirement::Sparse {
                index_ceiling: 0,
                idf_enabled: true,
            }
            .validate()
            .is_err()
        );
        assert!(
            VectorRequirement::Sparse {
                index_ceiling: 8,
                idf_enabled: false,
            }
            .validate()
            .is_err()
        );
        assert!(
            VectorRequirement::Sparse {
                index_ceiling: 8,
                idf_enabled: true,
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn dense_dimensions_are_bounded_and_nonzero() {
        assert!(
            VectorRequirement::Dense {
                dimensions: 0,
                distance: DenseDistance::Cosine,
            }
            .validate()
            .is_err()
        );
        assert!(
            VectorRequirement::Dense {
                dimensions: MAX_DENSE_DIMENSIONS + 1,
                distance: DenseDistance::Cosine,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn value_validation_enforces_kind_finiteness_and_order() {
        let requirement = VectorRequirement::Sparse {
            index_ceiling: 4,
            idf_enabled: true,
        };
        assert!(
            VectorValue::Sparse {
                indices: vec![0, 1, 3],
                values: vec![1.0, 2.0, 3.0],
            }
            .validate(&requirement)
            .is_ok()
        );
        assert!(
            VectorValue::Sparse {
                indices: vec![0, 0],
                values: vec![1.0, 2.0],
            }
            .validate(&requirement)
            .is_err()
        );
        assert!(
            VectorValue::Sparse {
                indices: vec![0, 4],
                values: vec![1.0, 2.0],
            }
            .validate(&requirement)
            .is_err()
        );
        assert!(
            VectorValue::Sparse {
                indices: vec![0],
                values: vec![f32::NAN],
            }
            .validate(&requirement)
            .is_err()
        );
        assert!(
            VectorValue::Dense(vec![0.5; 4])
                .validate(&requirement)
                .is_err()
        );
        assert!(
            VectorValue::Sparse {
                indices: Vec::new(),
                values: Vec::new(),
            }
            .validate(&requirement)
            .is_err()
        );
    }

    #[test]
    fn dense_value_width_matches_exactly() {
        let requirement = VectorRequirement::Dense {
            dimensions: 3,
            distance: DenseDistance::Dot,
        };
        assert!(
            VectorValue::Dense(vec![1.0, 2.0, 3.0])
                .validate(&requirement)
                .is_ok()
        );
        assert!(
            VectorValue::Dense(vec![1.0, 2.0])
                .validate(&requirement)
                .is_err()
        );
    }

    #[test]
    fn sparse_values_refuse_one_past_the_stored_value_ceiling() {
        let value = VectorValue::Sparse {
            indices: (0..=MAX_STORED_VECTOR_VALUES).collect(),
            values: vec![1.0; MAX_STORED_VECTOR_VALUES as usize + 1],
        };
        let requirement = VectorRequirement::Sparse {
            index_ceiling: MAX_STORED_VECTOR_VALUES + 1,
            idf_enabled: true,
        };
        assert_eq!(
            value
                .validate(&requirement)
                .expect_err("stored value cap")
                .field(),
            "stored_vector_values"
        );
    }

    #[test]
    fn vector_name_rejects_bad_grammar_and_overlong_text() {
        assert!(VectorName::new("a").is_ok());
        assert!(VectorName::new("unit-sparse-1").is_ok());
        assert!(VectorName::new("lex_code_v1").is_ok());
        assert!(VectorName::new("Unit-Sparse").is_err());
        assert!(VectorName::new("unit--sparse").is_err());
        assert!(VectorName::new("-unit").is_err());
        assert!(VectorName::new("unit-").is_err());
        assert!(VectorName::new("").is_err());
        assert!(VectorName::new(&"u".repeat(MAX_VECTOR_NAME_BYTES)).is_ok());
        assert!(VectorName::new(&"u".repeat(MAX_VECTOR_NAME_BYTES + 1)).is_err());
    }

    #[test]
    fn closed_wire_round_trip_for_mode_and_distance() {
        for mode in VectorMode::ALL {
            assert_eq!(VectorMode::parse(mode.as_str()), Ok(mode));
        }
        for distance in DenseDistance::ALL {
            assert_eq!(DenseDistance::parse(distance.as_str()), Ok(distance));
        }
        assert!(VectorMode::parse("global").is_err());
        assert!(DenseDistance::parse("cos").is_err());
    }

    #[test]
    fn requirement_canonical_round_trip() {
        for requirement in [
            VectorRequirement::Sparse {
                index_ceiling: 2048,
                idf_enabled: true,
            },
            VectorRequirement::Dense {
                dimensions: 768,
                distance: DenseDistance::Cosine,
            },
        ] {
            let encoded = requirement.to_canonical_value().expect("encode");
            let decoded = VectorRequirement::from_canonical_value(encoded).expect("decode");
            assert_eq!(decoded, requirement);
        }
    }

    #[test]
    fn named_requirement_canonical_round_trip() {
        let named = sparse(2048);
        let encoded = named.to_canonical_value().expect("encode");
        let decoded = NamedVectorRequirement::from_canonical_value(encoded).expect("decode");
        assert_eq!(decoded, named);
    }

    #[test]
    fn malformed_requirement_decode_fails_closed() {
        let unknown_kind = codec::object([
            ("kind", codec::text("other").expect("text")),
            ("index_ceiling", CanonicalValue::U64(8)),
            ("idf_enabled", CanonicalValue::Bool(true)),
        ])
        .expect("object");
        let error =
            VectorRequirement::from_canonical_value(unknown_kind).expect_err("unknown kind");
        assert_eq!(error.kind(), ContractErrorKind::InvalidTaggedVariant);
        assert_eq!(error.field(), "vector_requirement.kind");

        let missing_kind = codec::object([
            ("index_ceiling", CanonicalValue::U64(8)),
            ("idf_enabled", CanonicalValue::Bool(true)),
        ])
        .expect("object");
        let error =
            VectorRequirement::from_canonical_value(missing_kind).expect_err("missing kind");
        assert_eq!(error.kind(), ContractErrorKind::MalformedPayload);
        assert_eq!(error.field(), "kind");
    }

    #[test]
    fn initial_mode_is_sparse_only() {
        assert_eq!(INITIAL_VECTOR_MODE, VectorMode::SparseOnly);
    }
}
