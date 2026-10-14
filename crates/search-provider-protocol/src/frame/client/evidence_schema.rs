//! Strict additive wire schema for the `retrieve_evidence@1` and
//! `orient_scope@1` contracts (#213).
//!
//! This module owns no planner, executor, adapter, validator or projection
//! logic. It is transport serialization for two new closed recipes that are
//! additive to the eleven-recipe v1 surface, and it reuses the shared bounded
//! schema kernel so unknown, missing, duplicate, reordered or otherwise
//! noncanonical fields are rejected instead of defaulted.
//!
//! The two new-generation recipes travel through a versioned request envelope
//! tag, never through `RecipeIdV1` or `RecipeBodyV1`. A caller therefore cannot
//! smuggle a heuristic free-text question into `find_text@1`, and no existing
//! v1 recipe byte or v1 test changes.

use search_contracts::evidence::{
    EvidenceCardV1, EvidenceGroup, EvidenceGroupKind, EvidenceHints, EvidenceIntent,
    EvidenceRelation, EvidenceRelationKind, ExactCheckTemplate, OrientScopeRequestV2,
    OrientScopeResultV2, OrientationCard, OrientationDimension, OrientationGroup,
    OrientationHints, PublicCardSource, PublicCurrentnessState, PublicScopeSelector,
    RecipeIdV2, RetrieveEvidenceRequestV2, RetrieveEvidenceResultV2,
};
use search_contracts::ids::{RepresentationId, UnitId};
use search_contracts::query::{PriorityClass, QueryExecutionBudget};
use search_contracts::source::{UnitKind, UnitOccurrence};

use crate::error::ProtocolError;
use super::wire::{Decoder, Encoder, Result, Schema, record};

/// Schema version of the additive new-generation recipe envelope.
pub const EVIDENCE_RECIPE_SCHEMA_VERSION: u16 = 2;

/// Independent response-body ceiling below the canonical transport frame cap.
pub(super) const MAX_EVIDENCE_SCHEMA_BYTES: usize = 1024 * 1024;

/// Additive new-generation request union carried by one authenticated request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceRecipeRequestV2 {
    /// One bounded `retrieve_evidence@1` request.
    RetrieveEvidence(RetrieveEvidenceRequestV2),
    /// One bounded `orient_scope@1` request.
    OrientScope(OrientScopeRequestV2),
}

impl EvidenceRecipeRequestV2 {
    /// Exact closed recipe identity of this request.
    #[must_use]
    pub const fn recipe_id(&self) -> RecipeIdV2 {
        match self {
            Self::RetrieveEvidence(_) => RecipeIdV2::RetrieveEvidence,
            Self::OrientScope(_) => RecipeIdV2::OrientScope,
        }
    }

    /// Validates the selected request body against its contract shape.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::RetrieveEvidence(value) => value.validate(),
            Self::OrientScope(value) => value.validate(),
        }
        .map_err(|_| ProtocolError::InvalidBody)
    }
}

/// Additive new-generation result union returned by one authenticated request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceRecipeResultV2 {
    /// One bounded `retrieve_evidence@1` result.
    RetrieveEvidence(RetrieveEvidenceResultV2),
    /// One bounded `orient_scope@1` result.
    OrientScope(OrientScopeResultV2),
}

impl EvidenceRecipeResultV2 {
    /// Exact closed recipe identity of this result.
    #[must_use]
    pub const fn recipe_id(&self) -> RecipeIdV2 {
        match self {
            Self::RetrieveEvidence(_) => RecipeIdV2::RetrieveEvidence,
            Self::OrientScope(_) => RecipeIdV2::OrientScope,
        }
    }

    /// Validates the selected result body against its contract shape.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::RetrieveEvidence(value) => value.validate(),
            Self::OrientScope(value) => value.validate(),
        }
        .map_err(|_| ProtocolError::InvalidBody)
    }
}

/// Declares the shared bounded schema for one closed additive wire vocabulary.
macro_rules! registry {
    ($($name:ident),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.text(self.as_str()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                Self::parse(&input.text(64)?).map_err(|_| ProtocolError::InvalidBody)
            }
        }
    )+ };
}

registry!(
    EvidenceIntent, OrientationDimension, EvidenceRelationKind, PublicCurrentnessState,
    EvidenceGroupKind, PriorityClass, RecipeIdV2, UnitKind,
);

impl Schema for UnitId {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.text(&self.to_string()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::parse(&input.text(36)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl Schema for RepresentationId {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.text(&self.to_string()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::parse(&input.text(36)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

record!(UnitOccurrence {
    unit_id, representation_id, unit_kind, ordinal, native_anchor, structural_identity,
    configuration_predicate,
});
record!(PublicCardSource { source_id, revision, representation_profile });
record!(EvidenceHints { authorized_source_ids, suggested_exact_handles });
record!(OrientationHints { authorized_source_ids, suggested_exact_handles });
record!(QueryExecutionBudget {
    priority_class, deadline_ms, max_scoring_legs, max_prefetch_candidates_per_leg,
    max_validated_candidates, max_source_read_bytes, max_exact_scan_items, max_exact_scan_bytes,
    max_materialized_result_bytes, max_cpu_ms, max_memory_bytes,
});
record!(EvidenceRelation { kind, evidence_handle });
record!(EvidenceCardV1 {
    card_id, evidence_role, source, unit, handle, bounded_preview, producer_assurance,
    freshness, currentness, lineage, relations, ambiguity_group, score_profile, trace_ref,
} => |value: &EvidenceCardV1| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(EvidenceGroup { group_id, kind, card_ids });
record!(ExactCheckTemplate { template_id, rationale, predicate, native_anchor });
record!(OrientationCard {
    card_id, dimension, source, unit, handle, producer_assurance, freshness, currentness, rank_key,
});
record!(OrientationGroup { group_id, dimension, card_ids });

impl Schema for PublicScopeSelector {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        self.validate().map_err(|_| ProtocolError::InvalidBody)?;
        match self {
            Self::WorkspaceRevision(value) => {
                output.tag("workspace_revision")?;
                value.put(output)?;
            }
            Self::CorpusRevision { corpus_id, membership_revision } => {
                output.tag("corpus_revision")?;
                output.open(b'{')?;
                let mut first = true;
                output.field(&mut first, "corpus_id")?;
                corpus_id.put(output)?;
                output.field(&mut first, "membership_revision")?;
                membership_revision.put(output)?;
                output.close(b'}')?;
            }
            Self::PortfolioRevision { portfolio_id, portfolio_revision } => {
                output.tag("portfolio_revision")?;
                output.open(b'{')?;
                let mut first = true;
                output.field(&mut first, "portfolio_id")?;
                portfolio_id.put(output)?;
                output.field(&mut first, "portfolio_revision")?;
                portfolio_revision.put(output)?;
                output.close(b'}')?;
            }
            Self::OverlayRevision(value) => {
                output.tag("overlay_revision")?;
                value.put(output)?;
            }
        }
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "workspace_revision" => Self::WorkspaceRevision(Schema::get(input)?),
            "corpus_revision" => {
                input.open(b'{')?;
                let mut first = true;
                input.field(&mut first, "corpus_id")?;
                let corpus_id = Schema::get(input)?;
                input.field(&mut first, "membership_revision")?;
                let membership_revision = Schema::get(input)?;
                input.close(b'}')?;
                Self::CorpusRevision { corpus_id, membership_revision }
            }
            "portfolio_revision" => {
                input.open(b'{')?;
                let mut first = true;
                input.field(&mut first, "portfolio_id")?;
                let portfolio_id = Schema::get(input)?;
                input.field(&mut first, "portfolio_revision")?;
                let portfolio_revision = Schema::get(input)?;
                input.close(b'}')?;
                Self::PortfolioRevision { portfolio_id, portfolio_revision }
            }
            "overlay_revision" => Self::OverlayRevision(Schema::get(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        value.validate().map_err(|_| ProtocolError::InvalidBody)?;
        Ok(value)
    }
}

record!(RetrieveEvidenceRequestV2 {
    scope, source_view, question, intent, requested_roles, optional_hints, profile_id, budgets,
    progressive,
} => |value: &RetrieveEvidenceRequestV2| value.validate().map_err(|_| ProtocolError::InvalidBody));

record!(OrientScopeRequestV2 {
    scope, source_view, question, dimensions, optional_hints, profile_id, budgets, progressive,
} => |value: &OrientScopeRequestV2| value.validate().map_err(|_| ProtocolError::InvalidBody));

record!(RetrieveEvidenceResultV2 {
    request_id, plan_id, plan_fingerprint, result_fence, resolved_scope, cards, groups,
    suggested_exact_checks, coverage, continuation,
} => |value: &RetrieveEvidenceResultV2| value.validate().map_err(|_| ProtocolError::InvalidBody));

record!(OrientScopeResultV2 {
    request_id, plan_id, plan_fingerprint, result_fence, resolved_scope, entry_points, groups,
    recommended_reading, suggested_exact_checks, ambiguities, coverage, continuation,
} => |value: &OrientScopeResultV2| value.validate().map_err(|_| ProtocolError::InvalidBody));

fn put_versioned(output: &mut Encoder, recipe: RecipeIdV2) -> Result<()> {
    output.open(b'{')?;
    let mut first = true;
    output.field(&mut first, "v")?;
    EVIDENCE_RECIPE_SCHEMA_VERSION.put(output)?;
    output.field(&mut first, "recipe")?;
    output.text(recipe.as_str())
}

fn open_versioned(bytes: &[u8]) -> Result<Decoder<'_>> {
    if bytes.is_empty() {
        return Err(ProtocolError::InvalidBody);
    }
    if bytes.len() > MAX_EVIDENCE_SCHEMA_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let mut input = Decoder::new(bytes);
    input.open(b'{')?;
    Ok(input)
}

fn read_versioned(input: &mut Decoder<'_>, expected: RecipeIdV2) -> Result<()> {
    let mut first = true;
    input.field(&mut first, "v")?;
    let version = <u16 as Schema>::get(input)?;
    if version != EVIDENCE_RECIPE_SCHEMA_VERSION {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    input.field(&mut first, "recipe")?;
    let tag = input.text(64)?;
    let recipe = RecipeIdV2::parse(&tag).map_err(|_| ProtocolError::InvalidBody)?;
    if recipe != expected {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(())
}

/// Encodes the exact `retrieve_evidence@1` request schema.
pub fn encode_retrieve_evidence_request(value: &RetrieveEvidenceRequestV2) -> Result<Vec<u8>> {
    let mut output = Encoder::new(MAX_EVIDENCE_SCHEMA_BYTES);
    put_versioned(&mut output, RecipeIdV2::RetrieveEvidence)?;
    value.put(&mut output)?;
    output.finish()
}

/// Decodes the exact `retrieve_evidence@1` request schema.
///
/// The versioned dispatch tag is mandatory, an unexpected or unversioned tag is
/// refused, and the decoded value is re-encoded before it is returned so an
/// alternate spelling cannot survive a round trip.
pub fn decode_retrieve_evidence_request(bytes: &[u8]) -> Result<RetrieveEvidenceRequestV2> {
    let mut input = open_versioned(bytes)?;
    read_versioned(&mut input, RecipeIdV2::RetrieveEvidence)?;
    let value = Schema::get(&mut input)?;
    input.close(b'}')?;
    input.finish()?;
    if encode_retrieve_evidence_request(&value)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(value)
}

/// Encodes the exact `orient_scope@1` request schema.
pub fn encode_orient_scope_request(value: &OrientScopeRequestV2) -> Result<Vec<u8>> {
    let mut output = Encoder::new(MAX_EVIDENCE_SCHEMA_BYTES);
    put_versioned(&mut output, RecipeIdV2::OrientScope)?;
    value.put(&mut output)?;
    output.finish()
}

/// Decodes the exact `orient_scope@1` request schema.
pub fn decode_orient_scope_request(bytes: &[u8]) -> Result<OrientScopeRequestV2> {
    let mut input = open_versioned(bytes)?;
    read_versioned(&mut input, RecipeIdV2::OrientScope)?;
    let value = Schema::get(&mut input)?;
    input.close(b'}')?;
    input.finish()?;
    if encode_orient_scope_request(&value)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(value)
}

/// Encodes one exact additive new-generation result schema.
pub fn encode_evidence_result(value: &EvidenceRecipeResultV2) -> Result<Vec<u8>> {
    let mut output = Encoder::new(MAX_EVIDENCE_SCHEMA_BYTES);
    put_versioned(&mut output, value.recipe_id())?;
    match value {
        EvidenceRecipeResultV2::RetrieveEvidence(result) => result.put(&mut output)?,
        EvidenceRecipeResultV2::OrientScope(result) => result.put(&mut output)?,
    }
    output.finish()
}

/// Decodes one exact additive new-generation result schema.
///
/// The versioned dispatch tag selects the family, an unexpected or unversioned
/// tag is refused, and the decoded value is re-encoded before it is returned so
/// alternate spellings cannot survive a round trip.
pub fn decode_evidence_result(bytes: &[u8]) -> Result<EvidenceRecipeResultV2> {
    let mut input = open_versioned(bytes)?;
    let mut first = true;
    input.field(&mut first, "v")?;
    let version = <u16 as Schema>::get(&mut input)?;
    if version != EVIDENCE_RECIPE_SCHEMA_VERSION {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    input.field(&mut first, "recipe")?;
    let tag = input.text(64)?;
    let recipe = RecipeIdV2::parse(&tag).map_err(|_| ProtocolError::InvalidBody)?;
    let value = match recipe {
        RecipeIdV2::RetrieveEvidence => {
            EvidenceRecipeResultV2::RetrieveEvidence(Schema::get(&mut input)?)
        }
        RecipeIdV2::OrientScope => {
            EvidenceRecipeResultV2::OrientScope(Schema::get(&mut input)?)
        }
    };
    input.close(b'}')?;
    input.finish()?;
    value.validate()?;
    if encode_evidence_result(&value)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(value)
}
