//! Package-owned schema and state machine for immutable source-content manifests.
//!
//! The daemon/source adapter reads retained bytes and computes BLAKE3. This
//! module receives only bounded identities, digests and lengths, renders the
//! frozen line format and enforces exact header/object/end accounting. It never
//! receives source bodies, paths or credentials.

use core::fmt;

use search_contracts::{
    Blake3Digest32, Sha256Digest32, SourceNamespaceId,
};
use sha2::{Digest, Sha256};

use super::{
    ControlError, MAX_BYTES, MAX_ROWS, SourceImportBinding, SourceImportCounts,
};

const CONTENT_PROFILE: &[u8] =
    b"eliot/source-content/v1;retained-plaintext;sha256-verified;blake3-256;no-normalization";

/// Exact header inputs for one immutable source-content manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceContentManifestHeader {
    /// Explicit inactive import target.
    pub target_namespace: SourceNamespaceId,
    /// Original legacy namespace digest.
    pub legacy_namespace: Sha256Digest32,
    /// Complete verified source-history snapshot.
    pub catalog_snapshot: Sha256Digest32,
    /// Exact source-mapping record-chain digest.
    pub source_plan: Sha256Digest32,
    /// Exact number of retained content objects expected.
    pub expected_objects: u64,
}

/// One content-free readback fact supplied after the source adapter hashes bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceContentObjectReadback {
    /// Original stable source identity digest.
    pub legacy_source_id: Sha256Digest32,
    /// Original stable revision identity digest.
    pub legacy_revision_id: Sha256Digest32,
    /// Legacy SHA-256 content digest already verified by source readback.
    pub content_sha256: Sha256Digest32,
    /// Exact plaintext byte length.
    pub byte_length: u64,
    /// BLAKE3-256 recomputed from the exact retained bytes.
    pub content_blake3: Blake3Digest32,
}

/// Final exact content-manifest accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceContentManifestSummary {
    /// Number of object rows.
    pub objects: u64,
    /// Sum of exact plaintext source byte lengths.
    pub source_bytes: u64,
}

/// Closed content-manifest encoding failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContentManifestEncodingError {
    /// Header identity or cardinality is invalid.
    InvalidHeader,
    /// Header/object/end methods were called out of order.
    InvalidState,
    /// Observed object count differs from the declared exact count.
    ObjectCountMismatch,
    /// The declared or observed object ceiling is exceeded.
    ObjectLimitExceeded,
    /// Source-byte accounting overflowed.
    SourceBytesExceeded,
}

impl SourceContentManifestEncodingError {
    /// Stable historical daemon reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidHeader | Self::InvalidState | Self::ObjectCountMismatch => {
                "DIRECT_CONTROL_READBACK_MISMATCH"
            }
            Self::ObjectLimitExceeded => "DIRECT_MIGRATION_MAPPING_LIMIT",
            Self::SourceBytesExceeded => "DIRECT_MIGRATION_BYTES_EXCEEDED",
        }
    }
}

impl fmt::Display for SourceContentManifestEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceContentManifestEncodingError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EncodingPhase {
    Header,
    Objects,
    Finished,
}

/// Stateful owner of the frozen source-content manifest line schema.
///
/// The encoder accepts only digest/length facts. It assigns contiguous object
/// ordinals and prevents duplicate headers, early endings or excess rows.
#[derive(Clone, Debug)]
pub struct SourceContentManifestEncoder {
    header: SourceContentManifestHeader,
    phase: EncodingPhase,
    objects: u64,
    source_bytes: u64,
}

impl SourceContentManifestEncoder {
    /// Starts one exact manifest.
    ///
    /// # Errors
    ///
    /// Returns [`SourceContentManifestEncodingError`] when the target is the
    /// zero sentinel or the declared object count exceeds the migration bound.
    pub fn new(
        header: SourceContentManifestHeader,
    ) -> Result<Self, SourceContentManifestEncodingError> {
        if header.target_namespace.as_bytes() == &[0; 16] {
            return Err(SourceContentManifestEncodingError::InvalidHeader);
        }
        if header.expected_objects > MAX_ROWS {
            return Err(SourceContentManifestEncodingError::ObjectLimitExceeded);
        }
        Ok(Self {
            header,
            phase: EncodingPhase::Header,
            objects: 0,
            source_bytes: 0,
        })
    }

    /// Renders the single exact header row.
    ///
    /// # Errors
    ///
    /// Returns [`SourceContentManifestEncodingError::InvalidState`] after the
    /// header has already been emitted.
    pub fn header_row(
        &mut self,
    ) -> Result<Vec<u8>, SourceContentManifestEncodingError> {
        if self.phase != EncodingPhase::Header {
            return Err(SourceContentManifestEncodingError::InvalidState);
        }
        self.phase = EncodingPhase::Objects;
        Ok(format!(
            concat!(
                "{{\"kind\":\"source_content_header\",\"schema\":\"eliot.source-content.v1\",",
                "\"target_namespace_id\":\"{}\",\"legacy_namespace_sha256\":\"{}\",",
                "\"catalog_snapshot_sha256\":\"{}\",\"source_plan_chain_sha256\":\"{}\",",
                "\"content_profile_sha256\":\"{}\",\"expected_objects\":{},",
                "\"content_digest_algorithm\":\"blake3_256\",\"cutover_authorized\":false}}\n"
            ),
            self.header.target_namespace,
            self.header.legacy_namespace,
            self.header.catalog_snapshot,
            self.header.source_plan,
            source_content_profile_digest(),
            self.header.expected_objects,
        )
        .into_bytes())
    }

    /// Renders one exact object row and advances contiguous accounting.
    ///
    /// # Errors
    ///
    /// Returns a typed error when called outside the object phase, when more
    /// rows are supplied than declared, or when byte accounting overflows.
    pub fn object_row(
        &mut self,
        object: SourceContentObjectReadback,
    ) -> Result<Vec<u8>, SourceContentManifestEncodingError> {
        if self.phase != EncodingPhase::Objects {
            return Err(SourceContentManifestEncodingError::InvalidState);
        }
        if self.objects >= self.header.expected_objects {
            return Err(SourceContentManifestEncodingError::ObjectCountMismatch);
        }
        let ordinal = self
            .objects
            .checked_add(1)
            .ok_or(SourceContentManifestEncodingError::ObjectLimitExceeded)?;
        let source_bytes = self
            .source_bytes
            .checked_add(object.byte_length)
            .ok_or(SourceContentManifestEncodingError::SourceBytesExceeded)?;
        self.objects = ordinal;
        self.source_bytes = source_bytes;

        Ok(format!(
            concat!(
                "{{\"kind\":\"source_content_readback\",\"ordinal\":{},",
                "\"legacy_source_id\":\"{}\",\"legacy_revision_id\":\"{}\",",
                "\"content_sha256\":\"{}\",\"byte_length\":{},\"content_blake3\":\"{}\"}}\n"
            ),
            ordinal,
            object.legacy_source_id,
            object.legacy_revision_id,
            object.content_sha256,
            object.byte_length,
            object.content_blake3,
        )
        .into_bytes())
    }

    /// Renders the single exact end row and returns final accounting.
    ///
    /// # Errors
    ///
    /// Returns a typed error unless the header was emitted and exactly the
    /// declared number of objects was observed.
    pub fn finish(
        &mut self,
    ) -> Result<(Vec<u8>, SourceContentManifestSummary), SourceContentManifestEncodingError> {
        if self.phase != EncodingPhase::Objects {
            return Err(SourceContentManifestEncodingError::InvalidState);
        }
        if self.objects != self.header.expected_objects {
            return Err(SourceContentManifestEncodingError::ObjectCountMismatch);
        }
        self.phase = EncodingPhase::Finished;
        let summary = SourceContentManifestSummary {
            objects: self.objects,
            source_bytes: self.source_bytes,
        };
        let row = format!(
            concat!(
                "{{\"kind\":\"source_content_end\",\"objects\":{},\"source_bytes\":{},",
                "\"legacy_sha256_verified\":true,\"blake3_computed_from_bytes\":true,",
                "\"stability_receipt_issued\":false,\"residency_authorized\":false}}\n"
            ),
            summary.objects, summary.source_bytes,
        )
        .into_bytes();
        Ok((row, summary))
    }
}

/// SHA-256 identity of the frozen source-content acquisition profile.
#[must_use]
pub fn source_content_profile_digest() -> Sha256Digest32 {
    Sha256Digest32::from_bytes(Sha256::digest(CONTENT_PROFILE).into())
}

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
        // Tuple comparison preserves quarantine-on-mismatch: every binding
        // field must match exactly or the manifest is rejected as foreign.
        if (
            self.target_namespace,
            self.legacy_namespace,
            self.catalog_snapshot,
            self.source_plan,
        ) != (
            binding.target_namespace,
            binding.legacy_namespace,
            binding.catalog_snapshot,
            binding.plan_chain,
        ) {
            return Err(ControlError::IdentityMismatch);
        }
        if self.manifest_bytes == 0
            || self.manifest_bytes > MAX_BYTES
            || self.objects < binding.sources
            || self.objects > binding.events
            || (self.objects == 0) != (binding.events == 0)
            || self
                .objects
                .checked_mul(64 * 1024 * 1024)
                .is_none_or(|maximum| self.source_bytes > maximum)
        {
            return Err(ControlError::BudgetExceeded);
        }
        let mut bytes = Vec::with_capacity(208);
        bytes.extend_from_slice(b"ELSCREF1");
        bytes.extend_from_slice(self.target_namespace.as_bytes());
        for digest in [
            self.legacy_namespace,
            self.catalog_snapshot,
            self.source_plan,
            self.profile,
            self.manifest_chain,
        ] {
            bytes.extend_from_slice(digest.as_bytes());
        }
        for number in [self.manifest_bytes, self.objects, self.source_bytes] {
            bytes.extend_from_slice(&number.to_be_bytes());
        }
        Ok(bytes)
    }

    pub(super) const fn validate_counts(
        self,
        counts: SourceImportCounts,
    ) -> Result<(), ControlError> {
        // A/B/A may reuse an object, but a unique object must have at least one
        // occurrence. Empty files legitimately contribute zero source bytes.
        if self.objects < counts.sources || self.objects > counts.occurrences {
            return Err(ControlError::TransactionConflict);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
