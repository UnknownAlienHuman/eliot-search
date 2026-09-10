//! Loss/assurance taxonomy and monotonic assurance derivation.
//!
//! Assurance only moves downward: exact UTF-8 bytes outrank exact transcoding,
//! which outranks normalized output with recorded loss. Anything malformed,
//! truncated or over budget never becomes a product at all — it is a typed
//! error. Callers cannot upgrade assurance; derivation recomputes it from
//! loss evidence on every call.

use crate::MaterializationError;
use crate::maps::{LossKind, LossMap, MapBundle};
use crate::profile::ValidatedMaterializerProfile;

/// Maximum assurance allowed by exactness, completeness and loss evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssuranceCeiling {
    /// Byte-identical UTF-8 output with no recorded loss.
    ExactBytes,
    /// Exact UTF-16 transcode with no other recorded loss.
    ExactTranscoded,
    /// Output with BOM removal and/or newline normalization, all recorded.
    NormalizedWithRecordedLoss,
}

impl AssuranceCeiling {
    /// Stable short name used in receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactBytes => "exact-bytes",
            Self::ExactTranscoded => "exact-transcoded",
            Self::NormalizedWithRecordedLoss => "normalized-with-recorded-loss",
        }
    }

    /// Monotone rank: higher means stronger assurance. Lowering is the only
    /// direction derivation ever takes.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::ExactBytes => 3,
            Self::ExactTranscoded => 2,
            Self::NormalizedWithRecordedLoss => 1,
        }
    }
}

/// Derived assurance bound with its evidence counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationAssurance {
    ceiling: AssuranceCeiling,
    loss_records: u64,
    warnings: u64,
}

impl MaterializationAssurance {
    /// Maximum allowed assurance ceiling.
    #[must_use]
    pub const fn ceiling(&self) -> AssuranceCeiling {
        self.ceiling
    }

    /// Number of loss records backing this assurance.
    #[must_use]
    pub const fn loss_records(&self) -> u64 {
        self.loss_records
    }

    /// Number of surfaced warnings backing this assurance.
    #[must_use]
    pub const fn warnings(&self) -> u64 {
        self.warnings
    }
}

/// Computes the maximum allowed assurance ceiling from loss evidence alone.
///
/// Empty loss means exact bytes; transcoding-only loss means exact
/// transcoding; any BOM-removal or newline record means normalized output
/// with recorded loss.
#[must_use]
pub fn derive_assurance_ceiling(loss_map: &LossMap) -> AssuranceCeiling {
    let mut ceiling = AssuranceCeiling::ExactBytes;
    for record in loss_map.records() {
        let rank = match record.kind {
            LossKind::TranscodedEncoding => AssuranceCeiling::ExactTranscoded.rank(),
            LossKind::RemovedBom | LossKind::NormalizedNewline => {
                AssuranceCeiling::NormalizedWithRecordedLoss.rank()
            }
        };
        if rank < ceiling.rank() {
            ceiling = match record.kind {
                LossKind::TranscodedEncoding => AssuranceCeiling::ExactTranscoded,
                LossKind::RemovedBom | LossKind::NormalizedNewline => {
                    AssuranceCeiling::NormalizedWithRecordedLoss
                }
            };
        }
    }
    ceiling
}

/// Derives assurance from a validated map bundle, surfaced warnings and the
/// profile binding.
///
/// Loss without any surfaced warning is invisible degradation and fails with
/// [`MaterializationError::AssuranceViolation`]. The bundle must already be
/// structurally valid; use `validate_map_bundle` first.
pub fn derive_assurance(
    bundle: &MapBundle,
    warnings: u64,
    profile: &ValidatedMaterializerProfile,
) -> Result<MaterializationAssurance, MaterializationError> {
    if bundle.coordinate_map().profile_id() != profile.id()
        || bundle.loss_map().profile_id() != profile.id()
    {
        return Err(MaterializationError::ProfileMismatch);
    }
    let loss_records = u64::try_from(bundle.loss_map().records().len())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    if loss_records > 0 && warnings == 0 {
        return Err(MaterializationError::AssuranceViolation);
    }
    Ok(MaterializationAssurance {
        ceiling: derive_assurance_ceiling(bundle.loss_map()),
        loss_records,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::{LossMap, LossRecord};
    use crate::profile::baseline_profile_descriptor;
    use crate::profile::{MaterializerProfileId, validate_materializer_profile};
    use search_contracts::{NonZeroRevision, OpaqueId};

    fn profile() -> ValidatedMaterializerProfile {
        validate_materializer_profile(&baseline_profile_descriptor("assurance-test", 1))
            .expect("profile")
    }

    fn loss_map(kinds: &[LossKind]) -> LossMap {
        LossMap::for_test(
            kinds
                .iter()
                .map(|kind| LossRecord {
                    kind: *kind,
                    native_start: 0,
                    native_end: 1,
                    decoded_start: 0,
                    decoded_end: 1,
                    canonical_start: 0,
                    canonical_end: 1,
                })
                .collect(),
            profile().id(),
            OpaqueId::new("source:test").expect("source"),
            NonZeroRevision::new(1).expect("revision"),
        )
    }

    #[test]
    fn empty_loss_is_exact_bytes() {
        assert_eq!(
            derive_assurance_ceiling(&loss_map(&[])),
            AssuranceCeiling::ExactBytes
        );
    }

    #[test]
    fn transcode_only_is_exact_transcoded() {
        assert_eq!(
            derive_assurance_ceiling(&loss_map(&[LossKind::TranscodedEncoding])),
            AssuranceCeiling::ExactTranscoded
        );
    }

    #[test]
    fn any_recorded_normalization_lowers_to_floor() {
        assert_eq!(
            derive_assurance_ceiling(&loss_map(&[
                LossKind::TranscodedEncoding,
                LossKind::RemovedBom
            ])),
            AssuranceCeiling::NormalizedWithRecordedLoss
        );
        assert_eq!(
            derive_assurance_ceiling(&loss_map(&[LossKind::NormalizedNewline])),
            AssuranceCeiling::NormalizedWithRecordedLoss
        );
    }

    #[test]
    fn ceiling_order_is_monotone_downward() {
        assert!(AssuranceCeiling::ExactBytes.rank() > AssuranceCeiling::ExactTranscoded.rank());
        assert!(
            AssuranceCeiling::ExactTranscoded.rank()
                > AssuranceCeiling::NormalizedWithRecordedLoss.rank()
        );
    }

    #[test]
    fn profile_id_binding_is_public_but_opaque() {
        let _ = MaterializerProfileId::from_bytes([9; 32]);
    }
}
