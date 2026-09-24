//! All eleven P00 recipe inputs. No ad-hoc query blob or fallback recipe.

use search_contracts::{
    CompareImplementationsRecipe, CompileExactScanRecipe, CorpusDeltaRecipe, CorpusProfileRecipe,
    EditorPositionSelector, ExactCompletenessRequirements, ExactPredicate, ExecuteExactScanRecipe,
    ExpandHandleRecipe, ExpandHandleTarget, ExploreEntityRecipe, FindTextRecipe,
    InspectEntityRecipe, LocateRecipe, NormalizedNameSelector, PathSelector, ProvenanceRecipe,
    QualifiedSymbolSelector, RecipeBodyV1, RecipeIdV1, ReferencePortfolioScope, RelationKind,
    RequestedScope, SearchRecipeRequest, SubjectSelector,
};

use crate::error::ProtocolError;
use super::wire::{Decoder, Encoder, Result, Schema, record, tagged};

record!(ReferencePortfolioScope { portfolio_id, portfolio_revision });
tagged!(RequestedScope {
    ActiveWorkspace => "active_workspace",
    ExplicitMemberships => "explicit_memberships",
    Corpus => "corpus",
    ReferencePortfolio => "reference_portfolio",
    SourceHandle => "source_handle",
});
record!(EditorPositionSelector { workspace_id, buffer_snapshot_id, anchor });
record!(QualifiedSymbolSelector { normalized_symbol_key, entity_kinds });
record!(NormalizedNameSelector { name, entity_kinds });
record!(PathSelector { workspace_id, display_path });
tagged!(SubjectSelector {
    SourceHandle => "source_handle",
    EditorPosition => "editor_position",
    QualifiedSymbol => "qualified_symbol",
    NormalizedName => "normalized_name",
    Path => "path",
});
record!(ExactPredicate {
    kind, engine_and_version, serialized_form, input_domain, worst_case_complexity_class,
});
record!(ExactCompletenessRequirements {
    require_every_denominator_item, require_stable_or_retained_revision,
    require_current_observation, include_authenticated_unsaved_buffers,
    fail_on_timeout, fail_on_cancellation, fail_on_scope_drift,
});
record!(LocateRecipe { subject, evidence_roles });
record!(FindTextRecipe { predicate, case_policy, context_bytes_before, context_bytes_after });
record!(InspectEntityRecipe { subject, evidence_roles, include_relations }
    => |value: &InspectEntityRecipe| {
        // RECIPES.md permits configuration edges for exploration, not inspection.
        if value.include_relations.contains(&RelationKind::Configuration) {
            Err(ProtocolError::InvalidBody)
        } else { Ok(()) }
    });
record!(CompareImplementationsRecipe { subject, references, comparison_axes });
record!(ExploreEntityRecipe { subject, relation_kinds, max_depth });
record!(CorpusProfileRecipe { facets });
record!(CorpusDeltaRecipe { from_view, to_view, dimensions });
record!(ProvenanceRecipe { source_handle, max_lineage_depth });
record!(ExecuteExactScanRecipe { plan_ref });
tagged!(ExpandHandleTarget { Source => "source", Continuation => "continuation" });
record!(ExpandHandleRecipe { handle, expansion, max_bytes });
record!(SearchRecipeRequest {
    request_id, recipe, source_view, requested_scope, requested_budget_class, body,
} => |value: &SearchRecipeRequest| {
    if value.recipe != value.body.recipe_id() { return Err(ProtocolError::InvalidBody); }
    value.source_view.validate().map_err(|_| ProtocolError::InvalidBody)
});

impl Schema for RecipeBodyV1 {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        // The union key is the exact versioned registry value, not an alias.
        output.tag(self.recipe_id().as_str())?;
        match self {
            Self::Locate(value) => value.put(output)?,
            Self::FindText(value) => value.put(output)?,
            Self::InspectEntity(value) => value.put(output)?,
            Self::CompareImplementations(value) => value.put(output)?,
            Self::ExploreEntity(value) => value.put(output)?,
            Self::CorpusProfile(value) => value.put(output)?,
            Self::CorpusDelta(value) => value.put(output)?,
            Self::Provenance(value) => value.put(output)?,
            Self::CompileExactScan { predicate, body } => {
                // The Rust grouping is not an extra wire field. P00 puts the
                // predicate and completeness requirements beside each other.
                output.open(b'{')?;
                let mut first = true;
                output.field(&mut first, "predicate")?;
                predicate.put(output)?;
                output.field(&mut first, "completeness_requirements")?;
                body.completeness_requirements.put(output)?;
                output.close(b'}')?;
            }
            Self::ExecuteExactScan(value) => value.put(output)?,
            Self::ExpandHandle(value) => value.put(output)?,
        }
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let tag = input.tag()?;
        let kind = RecipeIdV1::parse_versioned(&tag).map_err(|_| ProtocolError::InvalidBody)?;
        let value = match kind {
            RecipeIdV1::Locate => Self::Locate(Schema::get(input)?),
            RecipeIdV1::FindText => Self::FindText(Schema::get(input)?),
            RecipeIdV1::InspectEntity => Self::InspectEntity(Schema::get(input)?),
            RecipeIdV1::CompareImplementations => Self::CompareImplementations(Schema::get(input)?),
            RecipeIdV1::ExploreEntity => Self::ExploreEntity(Schema::get(input)?),
            RecipeIdV1::CorpusProfile => Self::CorpusProfile(Schema::get(input)?),
            RecipeIdV1::CorpusDelta => Self::CorpusDelta(Schema::get(input)?),
            RecipeIdV1::Provenance => Self::Provenance(Schema::get(input)?),
            RecipeIdV1::CompileExactScan => {
                input.open(b'{')?;
                let mut first = true;
                input.field(&mut first, "predicate")?;
                let predicate = Schema::get(input)?;
                input.field(&mut first, "completeness_requirements")?;
                let completeness_requirements = Schema::get(input)?;
                input.close(b'}')?;
                Self::CompileExactScan { predicate, body: CompileExactScanRecipe { completeness_requirements } }
            }
            RecipeIdV1::ExecuteExactScan => Self::ExecuteExactScan(Schema::get(input)?),
            RecipeIdV1::ExpandHandle => Self::ExpandHandle(Schema::get(input)?),
        };
        input.close(b'}')?;
        Ok(value)
    }
}
