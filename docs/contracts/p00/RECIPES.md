# Recipe registry and typed request families

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
```

```yaml
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

## Recipe bodies and outputs

```yaml
locate@1:
  request:
    subject: SubjectSelector
    evidence_roles: bounded_set<EvidenceRole>
  output: SearchCandidateSet

find_text@1:
  request:
    predicate: ExactPredicate
    case_policy: exact | unicode_casefold
    context_bytes_before: u32
    context_bytes_after: u32
  output: SearchCandidateSet

inspect_entity@1:
  request:
    subject: SubjectSelector
    evidence_roles: bounded_set<EvidenceRole>
    include_relations: bounded_set<definition | reference | caller | test | documentation>
  output: EntityInspectionResult

compare_implementations@1:
  request:
    subject: SubjectSelector
    references:
      portfolio_id: ReferencePortfolioId
      portfolio_revision: PortfolioRevision
    comparison_axes: bounded_set<ComparisonAxis>
  output: CrossRepositoryBehaviorSet

explore_entity@1:
  request:
    subject: SubjectSelector
    relation_kinds: bounded_set<definition | reference | caller | test | documentation | configuration>
    max_depth: u8
  output: EntityExplorationResult

corpus_profile@1:
  request:
    facets: bounded_set<role | language_or_format | entity_kind | lineage | readiness>
  output: CorpusProfileResult

corpus_delta@1:
  request:
    from_view: SourceView
    to_view: SourceView
    dimensions: bounded_set<source | membership | representation | symbol | readiness>
  output: CorpusDeltaResult

provenance@1:
  request:
    source_handle: SearchSourceHandle
    max_lineage_depth: u8
  output: ProvenanceResult

compile_exact_scan@1:
  request:
    predicate: ExactPredicate
    completeness_requirements: ExactCompletenessRequirements
  output: ExactScanPlan

execute_exact_scan@1:
  request:
    plan_ref: ExactScanPlanRef
  output: ExactExecutionReport

expand_handle@1:
  request:
    handle: SearchSourceHandle | ContinuationHandle
    expansion: excerpt | source_metadata | provenance | continuation
    max_bytes: u64
  output: HandleExpansionResult
```

## Output minimums

Every output includes:

- request/plan identity where applicable;
- source/security/view generations;
- coverage and explicit gaps;
- bounded result size;
- typed public reason codes;
- no raw vendor cursor, collection, filter or reusable authorization decision.

Recipe-specific result records may add descriptive fields but cannot weaken these bindings.
