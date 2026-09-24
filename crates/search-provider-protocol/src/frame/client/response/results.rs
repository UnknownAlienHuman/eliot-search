//! All eleven P00 result families; no opaque catch-all or fabricated receipt.

use search_contracts::{
    AmbiguousSubjectCandidate, BehaviorComparison, ComparableImplementation,
    CompareImplementationsResult, ContinuationExpansion, CorpusChange, CorpusDeltaResult,
    CorpusFacet, CorpusProfileResult, CrossRepositoryBehaviorSet, EntityExplorationResult,
    EntityGraphEdge, EntityGraphNode, EntityInspectionResult, EvidenceObservation,
    ExcerptExpansion, HandleExpansionBody, HandleExpansionResult, LocalComparisonSubject,
    ProvenanceResult, ProvenanceStep, RecipeResultV1, ResolvedEntityExploration,
    ResolvedEntityInspection, ResolvedSubject, SourceMetadataExpansion,
    SubjectAmbiguityResult, SubjectAmbiguitySet,
};

use crate::error::ProtocolError;
use super::super::wire::{Decoder, Encoder, Result, Schema, record, tagged};

record!(ResolvedSubject {
    canonical_handle, match_basis, entity_kind, normalized_name,
    signature_observation, configuration_predicate,
});
record!(AmbiguousSubjectCandidate { source_handle, entity_kind, match_basis, disambiguation_summary }
    => |value: &AmbiguousSubjectCandidate| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(SubjectAmbiguitySet { requested_selector_digest, candidates, reason_code }
    => |value: &SubjectAmbiguitySet| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(SubjectAmbiguityResult { header, ambiguity });
record!(EvidenceObservation { role, source_handle, observation, assurance, configuration_predicate });
record!(ResolvedEntityInspection {
    header, subject, definitions, references, callers, tests, documentation,
    configuration_variants, continuation_handle,
});
record!(EntityGraphNode { node_id, source_handle, entity_kind, normalized_name });
record!(EntityGraphEdge { from_node_id, to_node_id, relation, assurance, evidence_handle });
record!(ResolvedEntityExploration {
    header, root_subject, nodes, edges, truncated_at_depth, continuation_handle,
});
record!(LocalComparisonSubject {
    resolved_subject, definition, signature, callers, tests, documentation,
});
record!(ComparableImplementation {
    lineage_id, match_basis, configuration_predicate, evidence_roles,
    behavior_signature, exact_handles,
});
record!(BehaviorComparison {
    shared_observations, variants, outliers, locally_absent_observations, conflicts, unknowns,
});
record!(CrossRepositoryBehaviorSet {
    header, local_subject, comparable_implementations, comparison, recommended_reading,
});
record!(CorpusFacet { dimension, value, count, count_assurance });
record!(CorpusProfileResult { header, scope, facets, readiness });
record!(CorpusChange {
    kind, source_id, source_membership_id, before_ref, after_ref, evidence_handles, assurance,
});
record!(CorpusDeltaResult { header, from_view, to_view, changes });
record!(ProvenanceStep { sequence, kind, input_refs, output_ref, profile_or_protocol_id, receipt_ref });
record!(ProvenanceResult { header, subject_handle, chain, unresolved_steps });
record!(ExcerptExpansion { source_revision_ref, native_anchor, content, content_digest, assurance });
record!(SourceMetadataExpansion {
    source_revision_ref, authorized_display_path, modality, language_or_format, provenance_ref,
});
record!(ContinuationExpansion { candidates, coverage_delta, next_continuation_handle });
record!(HandleExpansionResult { header, handle, authorization_receipt_ref, body });

// P00 explicitly wraps ambiguity/comparison/provenance bodies in a `result`
// field. Do not flatten these into their outer union or silently select a hit.
fn put_result<T: Schema>(value: &T, output: &mut Encoder) -> Result<()> {
    output.open(b'{')?;
    let mut first = true;
    output.field(&mut first, "result")?;
    value.put(output)?;
    output.close(b'}')
}

fn get_result<T: Schema>(input: &mut Decoder<'_>) -> Result<T> {
    input.open(b'{')?;
    let mut first = true;
    input.field(&mut first, "result")?;
    let value = T::get(input)?;
    input.close(b'}')?;
    Ok(value)
}

impl Schema for EntityInspectionResult {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::Resolved(value) => { output.tag("resolved")?; value.put(output)?; }
            Self::Ambiguous(value) => { output.tag("ambiguous")?; put_result(value, output)?; }
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "resolved" => Self::Resolved(Schema::get(input)?),
            "ambiguous" => Self::Ambiguous(get_result(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}

impl Schema for EntityExplorationResult {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::Resolved(value) => { output.tag("resolved")?; value.put(output)?; }
            Self::Ambiguous(value) => { output.tag("ambiguous")?; put_result(value, output)?; }
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "resolved" => Self::Resolved(Schema::get(input)?),
            "ambiguous" => Self::Ambiguous(get_result(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}

impl Schema for CompareImplementationsResult {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::Compared(value) => { output.tag("compared")?; put_result(value, output)?; }
            Self::Ambiguous(value) => { output.tag("ambiguous")?; put_result(value, output)?; }
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "compared" => Self::Compared(get_result(input)?),
            "ambiguous" => Self::Ambiguous(get_result(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}

impl Schema for HandleExpansionBody {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::Excerpt(value) => { output.tag("excerpt")?; value.put(output)?; }
            Self::SourceMetadata(value) => { output.tag("source_metadata")?; value.put(output)?; }
            Self::Provenance(value) => { output.tag("provenance")?; put_result(value, output)?; }
            Self::Continuation(value) => { output.tag("continuation")?; value.put(output)?; }
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "excerpt" => Self::Excerpt(Schema::get(input)?),
            "source_metadata" => Self::SourceMetadata(Schema::get(input)?),
            "provenance" => Self::Provenance(get_result(input)?),
            "continuation" => Self::Continuation(Schema::get(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}

// Result-union tags are those in RECIPE_RESULTS.md; the corresponding request
// retains its separate versioned RecipeIdV1 tag. No unregistered variant fits.
tagged!(RecipeResultV1 {
    Locate => "locate", FindText => "find_text", InspectEntity => "inspect_entity",
    CompareImplementations => "compare_implementations", ExploreEntity => "explore_entity",
    CorpusProfile => "corpus_profile", CorpusDelta => "corpus_delta", Provenance => "provenance",
    CompileExactScan => "compile_exact_scan", ExecuteExactScan => "execute_exact_scan",
    ExpandHandle => "expand_handle",
});
