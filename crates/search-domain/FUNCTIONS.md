# search-domain — source/caller audit and deletion map

## Gross production inventory

Public main `c8e38b6` contains **82,840 bytes (~80.9 KiB)** in `src/`:

| Module | Bytes | Current meaning |
|---|---:|---|
| `assurance.rs` | 3,471 | aggregate minimum assurance / reasons |
| `coverage.rs` | 5,342 | exact-report coverage/complete-negative classification |
| `currentness.rs` | 10,094 | multi-axis currentness and snapshot-drift policy |
| `decisions.rs` | 12,345 | generic decisions/reason sets and plan/snapshot fingerprints |
| `eligibility.rs` | 11,181 | set/AST eligibility, filter equivalence and digest proofs |
| `error.rs` | 2,340 | generic domain error |
| `lib.rs` | 2,220 | broad public re-export surface |
| `ordering.rs` | 4,380 | old candidate order over assurance/freshness/identity |
| `outcomes.rs` | 5,278 | generic mutation outcome/retry classification |
| `source_ownership_v2.rs` | 7,612 | source owner generation/cutover canonicalization |
| `transitions.rs` | 14,372 | source ownership, epoch and old publication state machines |
| `visibility.rs` | 4,205 | process/binding/grant/profile visibility booleans |

The package depends only on `search-contracts`, but that does not make the rules provider-neutral product
requirements. Most types are coupled to old contract families which are already scheduled for removal.

## Operation-level caller census

Repository searches at public main found no external call for:

- `evaluate_assurance` / `minimum_assurance`;
- `classify_coverage` / `proves_complete_negative`;
- `evaluate_currentness` / `classify_snapshot_drift`;
- `compute_plan_fingerprint` and query-snapshot fingerprint helpers;
- `build_base_eligibility_predicate` / `decide_eligibility` and filter-digest proofs;
- `stable_candidate_order` / `stable_sort_candidates`;
- `evaluate_visibility`.

Matches are package source/tests and documentation.

External current callers found:

### `transition_publication`

Used by `search-control-redb/src/persistent/publication*` and historical publication intent docs. It advances
the old `PublicationIntentState` ladder with seven guard axes and receipt references. The current target
replaces that whole model with one typed `PendingPublication`, actual Qdrant readback and one
`CurrentPublishedView` commit. Delete this transition with its contract types/callers.

### `transition_source_ownership` / `source_ownership_v2`

Used by old source-registry cutover and source-record namespace/currentness codecs/fixtures. Current source
ownership is one local root/source/current-revision model; universal namespace owner transfer is cancelled.
Delete these functions with the registry/record/cutover families.

### `classify_mutation_outcome`

Used by `search-runtime-owner/src/transition/health.rs` and daemon development owner code/tests. It maps five
caller booleans/enums to generic outcome/retryability. Concrete owners already need different action:

- redb uncertain commit → point-read affected row;
- Qdrant possible write → observe exact pending page state;
- object publication → verify exact final object;
- process launch/kill → observe owned child handle/state;
- known abort/pre-dispatch cancellation → known failure.

Move the small shared rule only if two final owners truly use the same API. Otherwise inline the direct
classification and remove this module.

### Cargo dependencies

Many packages list `search-domain.workspace = true`, including old protocol/access/handle/retention/eval/
overlay/planner packages already targeted for deletion. Some useful packages may import a type only through an
obsolete API or no longer use the dependency. Remove dependencies after compiling the current callers; do not
preserve this package to avoid Cargo.toml edits.

## Replacement rules by owner

### Scope and eligibility

`resolve_scope(requested_ids, pinned_view, optional_session)` performs the only useful rule:

- explicit unknown ID fails;
- explicit empty never widens;
- selected set intersects the pinned view and optional restricted subset;
- compatible sources share one retrieval/IDF population;
- the result is immutable for the request/window.

No generic eligibility AST, snapshot fence, overlap proof or filter digest.

### Currentness

- current source head is a typed redb row;
- published query currentness is one immutable `PublishedViewGuard`;
- a live window keeps that guard;
- source/view mismatch is an explicit stale/foreign/missing error at the owning lookup.

No axis set or fingerprint-only drift classifier.

### Coverage and assurance

- ranked top-k is explicitly non-exhaustive;
- exact scan alone may return complete-negative after scanning one pinned selected inventory;
- each card is backed by actual manifest/object/range verification;
- missing/unsupported/cancelled work produces explicit gaps/partial state.

No aggregate assurance proof or generic coverage denominator is needed beyond the current response fields.

### Ordering

- one Qdrant branch preserves Qdrant order;
- compatible branches use native fusion/grouping;
- a real non-Qdrant branch may use one small deterministic rank merge;
- evidence formatting never reorders.

Do not retain the old assurance/freshness/fingerprint candidate key.

### Visibility/readiness

A handler is available when it is constructed and its actual dependencies are ready. Restricted session
scope is checked directly. Remove process/binding/grant/profile Boolean visibility algebra.

### State transitions

Each effect owner validates its own small closed transition. Do not centralize unrelated source, publication,
process and credential state machines under one domain package.

## Deletion order

1. Move/remove publication transition with old `PublicationIntentState` contracts.
2. Move/remove source ownership v2/cutover with registry/record code.
3. Delete planner/currentness/eligibility/coverage/assurance/visibility/ordering modules after old callers and
   fixtures disappear.
4. Inline/move mutation outcome classification into remaining real effect owners.
5. Remove `search-domain` dependencies from actual packages and compile direct consumers.
6. Delete the crate/workspace member if no independently meaningful multi-consumer algorithm remains.

Do not create `search-domain-v2`, preserve re-export shims or keep generic enums for hypothetical future
operations.

## Focused verification

Verification belongs to owners:

- source compare-set/conflict/restart;
- publication pending/effect/readback/current-view/cleanup;
- scope never widens and IDF population is correct;
- exact scan truthful completeness;
- point→manifest→retained evidence;
- deterministic final result order;
- factual handler readiness;
- concrete unknown-effect readback.

Package-only property/golden tests for deleted proof APIs are removed, not ported.
