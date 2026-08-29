# Recipe registry and typed request families

Exact recipe outputs are defined in `RECIPE_RESULTS.md`.

## Exact registry

`RecipeIdV1` contains exactly:

```text
locate@1
find_text@1
inspect_entity@1
compare_implementations@1
explore_entity@1
corpus_profile@1
corpus_delta@1
provenance@1
compile_exact_scan@1
execute_exact_scan@1
expand_handle@1
```

No alias, unversioned spelling or vendor-specific recipe is accepted.

## Common request envelope

```yaml
SearchRecipeRequest:
  request_id: RequestId
  recipe: RecipeIdV1
  source_view: SourceView
  requested_scope: RequestedScope
  requested_budget_class: ProfileId
  body: RecipeBodyV1

RequestedScope:
  active_workspace: WorkspaceId
  explicit_memberships: bounded_list<SourceMembershipId>
  corpus: CorpusId
  reference_portfolio:
    portfolio_id: ReferencePortfolioId
    portfolio_revision: PortfolioRevision
  source_handle: SearchSourceHandle
```

Exactly one scope variant is present. Scope is intersected with the grant; it is never authority.

## Common selectors

```yaml
SubjectSelector:
  source_handle: SearchSourceHandle
  editor_position:
    workspace_id: WorkspaceId
    buffer_snapshot_id: BufferSnapshotId | null
    anchor: NativeAnchor
  qualified_symbol:
    normalized_symbol_key: BoundedSymbolKey
    entity_kinds: bounded_set<EntityKind>
  normalized_name:
    name: BoundedName
    entity_kinds: bounded_set<EntityKind>
  path:
    workspace_id: WorkspaceId
    display_path: BoundedDisplayPath
```

```text
EvidenceRole = definition | reference | test | documentation | caller | configuration
ComparisonAxis = interface | validation | errors | side_effects | tests | callers | documentation
```

## Recipe bodies

```yaml
locate@1:
  subject: SubjectSelector
  evidence_roles: bounded_set<EvidenceRole>

find_text@1:
  predicate: ExactPredicate
  case_policy: exact | unicode_casefold
  context_bytes_before: u32
  context_bytes_after: u32

inspect_entity@1:
  subject: SubjectSelector
  evidence_roles: bounded_set<EvidenceRole>
  include_relations: bounded_set<definition | reference | caller | test | documentation>

compare_implementations@1:
  subject: SubjectSelector
  references:
    portfolio_id: ReferencePortfolioId
    portfolio_revision: PortfolioRevision
  comparison_axes: bounded_set<ComparisonAxis>

explore_entity@1:
  subject: SubjectSelector
  relation_kinds: bounded_set<definition | reference | caller | test | documentation | configuration>
  max_depth: u8

corpus_profile@1:
  facets: bounded_set<role | language_or_format | entity_kind | lineage | readiness>

corpus_delta@1:
  from_view: SourceView
  to_view: SourceView
  dimensions: bounded_set<source | membership | representation | symbol | readiness>

provenance@1:
  source_handle: SearchSourceHandle
  max_lineage_depth: u8

compile_exact_scan@1:
  predicate: ExactPredicate
  completeness_requirements: ExactCompletenessRequirements

execute_exact_scan@1:
  plan_ref: ExactScanPlanRef

expand_handle@1:
  handle: SearchSourceHandle | ContinuationHandle
  expansion: excerpt | source_metadata | provenance | continuation
  max_bytes: u64
```

`RecipeBodyV1` is a tagged union keyed by `RecipeIdV1`; body/recipe mismatch fails contract validation.

## Output minimums

Every output includes request/plan identity where applicable, source/security/view fences, truthful
coverage, bounded size and typed reasons. No output contains a raw vendor cursor, collection, filter,
point ID or reusable authorization decision.
