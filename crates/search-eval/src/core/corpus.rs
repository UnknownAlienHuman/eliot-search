//! Immutable control-corpus model and structural validation.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder};
use super::limits::EvalLimits;

/// Closed control-case family.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CaseFamily {
    /// Locate a bounded source/entity target.
    Locate,
    /// Literal or lexical text retrieval.
    FindText,
    /// Inspect one resolved entity.
    InspectEntity,
    /// Explore a bounded entity neighborhood.
    ExploreEntity,
    /// Compare independent implementations.
    CompareImplementations,
    /// Compile or execute an exact scan.
    ExactScan,
    /// Describe one frozen corpus.
    CorpusProfile,
    /// Compare two frozen corpus revisions.
    CorpusDelta,
    /// Produce source provenance.
    Provenance,
    /// Expand an opaque source handle.
    HandleExpansion,
    /// Fork relationship fixture.
    Fork,
    /// Mirror/copy relationship fixture.
    Mirror,
    /// Nested repository boundary fixture.
    NestedRepository,
    /// Submodule boundary fixture.
    Submodule,
    /// Restrictive access and purge fixture.
    Security,
    /// Crash and mutation-recovery fixture.
    Recovery,
    /// Framing/replay/cancellation fixture.
    Protocol,
    /// Latency and resource fixture.
    Performance,
}

impl CaseFamily {
    /// Topology families required by the architecture.
    pub const REQUIRED_TOPOLOGY: [Self; 4] = [
        Self::Fork,
        Self::Mirror,
        Self::NestedRepository,
        Self::Submodule,
    ];
}

/// One immutable oracle-bearing control case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCase {
    /// Stable case identity.
    pub case_id: OpaqueId,
    /// Closed case family.
    pub family: CaseFamily,
    /// Exact public fixture digest.
    pub fixture_digest: Blake3Digest32,
    /// Exact private oracle digest.
    pub oracle_digest: Blake3Digest32,
    /// Independent repository lineage represented by this case.
    pub lineage_id: OpaqueId,
    /// Whether fixture bytes are immutable/content-addressed.
    pub immutable: bool,
    /// Whether the oracle remains isolated from candidate-visible state.
    pub oracle_private: bool,
    /// Finite input ceiling.
    pub max_input_bytes: u64,
    /// Finite raw-output ceiling.
    pub max_output_bytes: u64,
    /// Content-free fixture evidence.
    pub fixture_receipt: ReceiptRef,
}

/// Exact registered control corpus manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCorpusManifest {
    /// Stable corpus identity.
    pub corpus_id: OpaqueId,
    /// Monotone corpus revision.
    pub revision: u64,
    /// Digest of the complete canonical corpus manifest.
    pub manifest_digest: Blake3Digest32,
    /// Digest of the exact fixture index.
    pub fixture_index_digest: Blake3Digest32,
    /// Digest of the disclosure policy.
    pub disclosure_policy_digest: Blake3Digest32,
    /// Every case in deterministic canonical order.
    pub cases: Vec<ControlCase>,
    /// Case families declared mandatory for this run profile.
    pub mandatory_families: BTreeSet<CaseFamily>,
    /// Whether the manifest itself is immutable.
    pub immutable: bool,
    /// Content-free manifest receipt.
    pub manifest_receipt: ReceiptRef,
}

/// Control corpus that passed structural, topology, lineage, and oracle checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedControlCorpus {
    manifest: ControlCorpusManifest,
    lineage_count: usize,
    family_counts: BTreeMap<CaseFamily, usize>,
    validation_digest: Blake3Digest32,
}

impl ValidatedControlCorpus {
    /// Exact accepted manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ControlCorpusManifest {
        &self.manifest
    }

    /// Number of independent represented lineages.
    #[must_use]
    pub const fn lineage_count(&self) -> usize {
        self.lineage_count
    }

    /// Number of cases in one family.
    #[must_use]
    pub fn family_count(&self, family: CaseFamily) -> usize {
        self.family_counts.get(&family).copied().unwrap_or(0)
    }

    /// Deterministic validation fingerprint.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates the registered control corpus without executing a candidate.
pub fn validate_control_corpus(
    manifest: ControlCorpusManifest,
    limits: EvalLimits,
) -> Result<ValidatedControlCorpus, EvalError> {
    let limits = limits.validate()?;
    if manifest.revision == 0
        || !manifest.immutable
        || manifest.cases.is_empty()
        || manifest.cases.len() > limits.max_cases
        || manifest.mandatory_families.is_empty()
    {
        return Err(EvalError::CorpusInvalid);
    }

    let mut case_ids = BTreeSet::new();
    let mut lineages = BTreeSet::new();
    let mut family_counts = BTreeMap::new();
    for case in &manifest.cases {
        if !case_ids.insert(case.case_id.clone()) {
            return Err(EvalError::DuplicateCase);
        }
        if !case.immutable
            || !case.oracle_private
            || case.max_input_bytes == 0
            || case.max_output_bytes == 0
            || case.fixture_receipt.as_str().is_empty()
        {
            return Err(if case.oracle_private {
                EvalError::CorpusInvalid
            } else {
                EvalError::OracleContamination
            });
        }
        lineages.insert(case.lineage_id.clone());
        *family_counts.entry(case.family).or_insert(0_usize) += 1;
    }
    if lineages.len() < 8 || lineages.len() > limits.max_lineages {
        return Err(EvalError::InsufficientLineages);
    }
    for family in manifest
        .mandatory_families
        .iter()
        .copied()
        .chain(CaseFamily::REQUIRED_TOPOLOGY)
    {
        if family_counts.get(&family).copied().unwrap_or(0) == 0 {
            return Err(EvalError::CorpusIncomplete);
        }
    }
    if manifest
        .cases
        .windows(2)
        .any(|pair| pair[0].case_id >= pair[1].case_id)
    {
        return Err(EvalError::CorpusInvalid);
    }

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/corpus-validation/v1");
    fingerprint.push_text(manifest.corpus_id.as_str());
    fingerprint.push_u64(manifest.revision);
    fingerprint.push_digest(manifest.manifest_digest);
    fingerprint.push_digest(manifest.fixture_index_digest);
    fingerprint.push_digest(manifest.disclosure_policy_digest);
    for case in &manifest.cases {
        fingerprint.push_text(case.case_id.as_str());
        fingerprint.push_u64(case_family_tag(case.family));
        fingerprint.push_digest(case.fixture_digest);
        fingerprint.push_digest(case.oracle_digest);
        fingerprint.push_text(case.lineage_id.as_str());
        fingerprint.push_u64(case.max_input_bytes);
        fingerprint.push_u64(case.max_output_bytes);
    }
    Ok(ValidatedControlCorpus {
        manifest,
        lineage_count: lineages.len(),
        family_counts,
        validation_digest: fingerprint.finish(),
    })
}

const fn case_family_tag(value: CaseFamily) -> u64 {
    match value {
        CaseFamily::Locate => 1,
        CaseFamily::FindText => 2,
        CaseFamily::InspectEntity => 3,
        CaseFamily::ExploreEntity => 4,
        CaseFamily::CompareImplementations => 5,
        CaseFamily::ExactScan => 6,
        CaseFamily::CorpusProfile => 7,
        CaseFamily::CorpusDelta => 8,
        CaseFamily::Provenance => 9,
        CaseFamily::HandleExpansion => 10,
        CaseFamily::Fork => 11,
        CaseFamily::Mirror => 12,
        CaseFamily::NestedRepository => 13,
        CaseFamily::Submodule => 14,
        CaseFamily::Security => 15,
        CaseFamily::Recovery => 16,
        CaseFamily::Protocol => 17,
        CaseFamily::Performance => 18,
    }
}
