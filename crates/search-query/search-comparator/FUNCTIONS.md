# Function contract — `search-comparator`

**Status:** W6/P11 logical contract; no comparison implementation exists yet.

The comparator aligns source-backed observations across independently resolved implementations. It
produces a descriptive evidence matrix with variants, conflicts and unknowns. It never decides which
implementation is correct, canonical, preferred or suitable for adoption.

## State and authority

The package owns no durable state, retrieval engine, source bytes or handle table. It consumes:

- one accepted local `SubjectResolution`;
- bounded comparable implementation observations validated against exact source revisions;
- repository-lineage/fork/mirror relation receipts;
- evidence-role, configuration-predicate and assurance observations;
- a closed comparison-axis/policy profile.

It owns normalization, lineage collapse, behavior-signature alignment, evidence-role analysis,
comparison classification, coverage and recommended-reading ordering.

## `validate_comparison_request`

```text
validate_comparison_request(request, local_resolution, source_view, portfolio_revision, grant)
    -> Result<ValidatedComparisonRequest, CompareError>
```

Requires a uniquely resolved or explicitly selected local subject, immutable reference portfolio
revision, authorized source view, closed bounded axes and disclosure/budget limits.

An unresolved `AMBIGUOUS` subject is returned to the caller; the comparator does not select one. Empty
reference scope, stale source view, unsupported axis or client-authored vendor filter fails explicitly.

## `validate_comparable_implementation`

```text
validate_comparable_implementation(candidate, local_subject, evidence_receipts, context)
    -> Result<ComparableImplementation, CompareError>
```

Requires exact source/revision handles, validated definition or subject occurrence, match basis,
repository lineage identity, structural/lexical profile/assurance, configuration predicate and bounded
evidence-role handles.

Same name alone is insufficient. A candidate may be comparable through qualified identity, accepted
signature/entity-kind compatibility, structural analogue or explicitly admitted lexical/semantic basis.
The basis and uncertainty remain visible.

## `normalize_behavior_signature`

```text
normalize_behavior_signature(implementation, axis_profile, limits)
    -> Result<BehaviorSignature, CompareError>
```

Builds a closed, deterministic descriptive signature across requested axes:

```text
interface
validation
errors
side_effects
tests
callers
documentation
```

Every component references source-backed observations/handles, exact configuration predicate,
assurance and extractor profile. Missing evidence remains `unknown`; it is not converted to empty,
absent or false. Natural-language summaries are bounded templates/observations, not hidden model
synthesis.

## `collapse_repository_lineages`

```text
collapse_repository_lineages(implementations, lineage_receipts, limits)
    -> Result<LineageGroupSet, CompareError>
```

Forks, mirrors and proven copies of one implementation count as one independent lineage. Collapse
requires accepted lineage/copy receipts and preserves member handles for navigation.

Ambiguous lineage remains separate with `LINEAGE_AMBIGUOUS`; it cannot be optimistically collapsed or
counted as confidently independent. Group ordering and representative choice are deterministic and
content-neutral.

## `align_evidence_roles`

```text
align_evidence_roles(group, axis_profile, limits)
    -> Result<EvidenceRoleAlignment, CompareError>
```

Aligns definition, test, caller, documentation and configuration evidence without treating any role as
automatic truth.

- definitions describe implementation facts at their assurance ceiling;
- tests describe asserted/examples behavior, not proof the implementation satisfies it;
- callers describe observed usage patterns;
- documentation describes stated intent/contract;
- configuration predicates delimit applicability.

Contradiction between roles becomes an explicit conflict or unknown, never silently reconciled.

## `partition_configuration_variants`

```text
partition_configuration_variants(observations, predicate_context, limits)
    -> Result<ConfigurationVariantSet, CompareError>
```

Preserves exact `cfg`/feature/target predicates. Mutually exclusive variants are separate variants, not
conflicts. Overlapping predicates with incompatible observations may form a conflict. Unknown predicates
remain unknown and cannot be treated as universally active.

The comparator does not evaluate predicates without an explicit accepted feature/target context.

## `compare_axes`

```text
compare_axes(local, lineage_groups, aligned_roles, axis_profile, limits)
    -> Result<ComparisonMatrix, CompareError>
```

For each axis, returns bounded source-backed categories:

```text
shared_observations
variants
outliers
locally_absent_observations
conflicts
unknowns
```

Classification rules are deterministic and profile-versioned. `locally_absent` means absent from the
validated local evidence under the stated coverage; it is not complete negative proof unless an exact
report is attached. An outlier is descriptive frequency/lineage evidence, not an error or inferiority
claim.

Independent counts are computed after lineage collapse. Five forks never become five independent votes.

## `classify_conflict`

```text
classify_conflict(left, right, axis, predicate_relation, assurance)
    -> BehaviorComparisonDecision
```

Closed decisions:

- `EQUIVALENT_OBSERVATION`
- `CONFIGURATION_VARIANT`
- `MATERIAL_VARIANT`
- `CONFLICT`
- `INSUFFICIENT_ASSURANCE`
- `UNKNOWN`

A conflict requires overlapping applicability and source-backed incompatible observations. Different
assurance ceilings or missing evidence usually produce `INSUFFICIENT_ASSURANCE`/`UNKNOWN`, not a forced
conflict.

## `compute_comparison_coverage`

```text
compute_comparison_coverage(request, candidate_receipts, lineage_groups, role_alignment, gaps)
    -> ComparisonCoverage
```

Reports requested/executed axes, represented and omitted memberships/lineages, candidate match bases,
role coverage, configuration coverage, observation freshness, failed/degraded providers, ambiguity and
unknowns.

Top-k candidate scope is never relabeled complete repository scope. Comparison cannot imply exhaustive
absence or consensus from incomplete portfolio/axis evidence.

## `order_recommended_reading`

```text
order_recommended_reading(matrix, handles, limits) -> RecommendedReading
```

Orders existing authorized source handles; it does not mint or authorize handles. Baseline ordering:

```text
local definition/contract
> decisive conflict/variant evidence
> independently corroborating definitions
> tests/callers demonstrating behavior
> documentation
> representative unknown/gap evidence
```

Then assurance, evidence-role priority, portfolio priority, lineage diversity, source identity and
native coordinate. Per-lineage/source caps prevent mirror/fork domination. Raw vendor scores are not
compared across populations.

## `assemble_behavior_set`

```text
assemble_behavior_set(request, local, groups, matrix, coverage, reading, receipts)
    -> Result<CrossRepositoryBehaviorSet, CompareError>
```

Binds local subject receipt, source/reference portfolio revisions, plan/security/profile generations,
lineage policy/profile, matrix/coverage/reading digests and validation receipts.

Output contains no `correct`, `best`, `recommended_implementation`, adoption decision, hidden confidence
score or client admission disposition. Unknowns and material disagreements remain first-class.

## `revalidate_comparison`

```text
revalidate_comparison(result, current_fence, current_portfolio) -> ComparisonRevalidation
```

Invalidates/requires recomputation when local subject resolution, source owner/access/purge/shadow,
reference portfolio, lineage relation, evidence/profile, observation freshness or handle validity drifts.
A restrictive security change overrides the result immediately.

## Cancellation, deadlines and retry

All comparison operations are pure over bounded captured inputs. Cancellation/budget exhaustion returns
an incomplete comparison with explicit omitted axes/lineages only when the result contract permits;
it never fabricates full coverage or a normative conclusion. Repeating equal inputs is deterministic.
There is no durable mutation or unknown external commit outcome.

## Typed failures and reasons

- `AMBIGUOUS_SUBJECT`
- `COMPARISON_SCOPE_EMPTY`
- `COMPARISON_REQUEST_INVALID`
- `COMPARISON_CONTEXT_STALE`
- `COMPARABLE_EVIDENCE_INVALID`
- `INSUFFICIENT_COMPARABLE_EVIDENCE`
- `LINEAGE_AMBIGUOUS`
- `LINEAGE_RECEIPT_STALE`
- `CONFIGURATION_AMBIGUOUS`
- `COMPARISON_AXIS_UNSUPPORTED`
- `COMPARISON_BUDGET_EXHAUSTED`
- `COMPARISON_CANCELLED`
- `INCOMPLETE_COVERAGE`
- `HANDLE_UNAVAILABLE`
- `NORMATIVE_VERDICT_FORBIDDEN`

## Required tests / qualification evidence

- ambiguous local subject blocks comparison until explicit resolution/selection;
- renamed true analogue via accepted match basis; false same-name stays separate;
- forks/mirrors/copies collapse to one independent lineage;
- ambiguous lineage is neither collapsed nor counted confidently independent;
- definitions/tests/callers/docs remain distinct evidence roles;
- decisive test/doc contradiction becomes conflict/unknown, not automatic truth selection;
- mutually exclusive cfg variants are variants, not conflicts;
- overlapping incompatible predicates create bounded source-backed conflict;
- unknown cfg predicate is not unconditional;
- missing local observation is not complete absence without exact proof;
- incomplete top-k/portfolio coverage remains explicit;
- recommended reading preserves lineage diversity and deterministic order;
- equal inputs produce byte-identical matrix/coverage/result digests;
- cancellation/budget reports omissions without verdict;
- output/API contains no correctness/best/adoption claim, raw vendor type/score or hidden source dump;
- access/purge/portfolio/profile/subject drift invalidates the result.
