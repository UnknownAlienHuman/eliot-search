//! An immutable manifest reference joining imported lineage to verified content facts.
//! The producer owns BLAKE3 readback. Storing this binding does not grant residency,
//! authenticate source bytes by itself, or activate an imported namespace.

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use super::{ControlError, MAX_BYTES, SourceImportBinding, SourceImportCounts};

/// Exact content-manifest binding for an inactive source-map target.
///
/// The manifest remains outside redb; this fixed-size record contains no source
/// bodies, point sets, paths or credentials. Its chain is the manifest producer's
/// SHA-256 record-chain fingerprint, not raw-file SHA-256 or content BLAKE3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceContentManifest {
    /// Explicit imported namespace shared by the source mapping and manifest.
    pub target_namespace: SourceNamespaceId,
    /// Original namespace fingerprint, never relabelled as a canonical UUID.
    pub legacy_namespace: Sha256Digest32,
    /// Complete source-history snapshot used to produce both artifacts.
    pub catalog_snapshot: Sha256Digest32,
    /// Exact source-map record chain referenced by the content manifest.
    pub source_plan: Sha256Digest32,
    /// Exact content acquisition/hash profile, including its digest algorithm.
    pub profile: Sha256Digest32,
    /// Immutable content-manifest record-chain identity.
    pub manifest_chain: Sha256Digest32,
    /// Exact encoded manifest length verified by the producer.
    pub manifest_bytes: u64,
    /// Distinct retained content objects, not source revision occurrences.
    pub objects: u64,
    /// Total plaintext source bytes represented by those objects.
    pub source_bytes: u64,
}

impl SourceContentManifest {
    pub(super) fn encode(self, binding: SourceImportBinding) -> Result<Vec<u8>, ControlError> {
        if self.target_namespace != binding.target_namespace
            || self.legacy_namespace != binding.legacy_namespace
            || self.catalog_snapshot != binding.catalog_snapshot
            || self.source_plan != binding.plan_chain
        {
            return Err(ControlError::IdentityMismatch);
        }
        if self.manifest_bytes == 0 || self.manifest_bytes > MAX_BYTES
            || self.objects < binding.sources || self.objects > binding.events
            || (self.objects == 0) != (binding.events == 0)
            || self.objects.checked_mul(64 * 1024 * 1024)
                .is_none_or(|maximum| self.source_bytes > maximum)
        {
            return Err(ControlError::BudgetExceeded);
        }
        let mut bytes = Vec::with_capacity(208);
        bytes.extend_from_slice(b"ELSCREF1");
        bytes.extend_from_slice(self.target_namespace.as_bytes());
        for digest in [self.legacy_namespace, self.catalog_snapshot, self.source_plan,
            self.profile, self.manifest_chain]
        {
            bytes.extend_from_slice(digest.as_bytes());
        }
        for number in [self.manifest_bytes, self.objects, self.source_bytes] {
            bytes.extend_from_slice(&number.to_be_bytes());
        }
        Ok(bytes)
    }

    pub(super) fn validate_counts(self, counts: SourceImportCounts) -> Result<(), ControlError> {
        // A/B/A may reuse an object, but a unique object must have at least one
        // occurrence. Empty files legitimately contribute zero source bytes.
        if self.objects < counts.sources || self.objects > counts.occurrences {
            return Err(ControlError::TransactionConflict);
        }
        Ok(())
    }
}
