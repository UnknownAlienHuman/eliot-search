# Function contract — `search-access`

**Status:** W4/P08 logical contract; security mutation hardening continues in W7/P13.

The package compiles server-authoritative eligibility before retrieval and owns the live restrictive
security fence. It exposes no Qdrant filter, collection, point ID or reusable authorization decision.

## Grant and scope operations

### `validate_grant(claims, binding, installation, now) -> Result<ValidatedGrant, AccessError>`

Validates signature/pairing, installation/incarnation/binding, issued boot, expiry, nonce,
revocation generation, recipe family, modality, budget class and disclosure/source-read ceilings.
The caller supplies time. Ordinary validation is read-only and creates no durable lease/idempotency row.

### `intersect_scope(requested, grant, authoritative_snapshot) -> Result<AuthorizedScope, AccessError>`

Intersects requested corpus/portfolio/membership scope with the immutable server registry snapshot.
Unknown, stale or foreign identifiers never widen the result. Empty authorized scope is a typed result.

### `compile_base_eligibility(scope, route, snapshot) -> Result<BaseEligibilityPlan, AccessError>`

Builds the closed vendor-neutral predicate over installation, collection generation, one projection
membership, access/scoring partition, epoch validity, live deny/purge and overlay shadow state. The
canonical predicate digest is shared by retrieval and filtered-IDF population.

### `compile_safe_legs(scope, routes, overlap_proofs, budget) -> Result<BoundedList<SafeRetrievalLeg>, AccessError>`

Each leg has one coherent access/scoring population. Grouping equivalent memberships requires a current
`OverlapFreeRouteProof` binding route, owner, membership, access, generation and profile revisions.
Absent or stale proof yields separate bounded legs or explicit exhaustion; it never emits an unbounded
per-file repair filter.

## Live security operations

### `begin_security_mutation(command, operation_id, control) -> Result<SecurityMutationGuard, AccessError>`

Acquires the domain barrier and validates monotonic restrictive generation. Same operation ID and same
canonical command is retry-safe; same ID with different input is rejected.

### `commit_and_publish_restriction(guard, ports, context) -> Result<SecurityMutationReceipt, AccessError>`

Orders: durable control commit → immutable live-deny snapshot publication → dependent invalidation
(query legs, handles, continuations, caches) → acknowledgement. Once durable commit may have occurred,
cancellation cannot report rollback; recovery completes publication/invalidation or leaves the domain
`FAIL_CLOSED`.

### `recheck_live_access(request_fence, live_state, checkpoint) -> Result<AccessPermit, AccessError>`

Required checkpoints are request admission, before each leg dispatch, after leg completion, before
source readback, before result emission and every handle/continuation expansion. Restrictive live state
overrides the planned snapshot immediately.

### `classify_contaminated_legs(execution, previous, current) -> ContaminationDecision`

Any leg whose candidate population, IDF, counts, diversity or trace was influenced by newly denied or
purged material is discarded as a whole. Candidate-only cleanup cannot preserve its ordering.

## Failure, cancellation and bounds

All lists, legs and proofs are bounded by accepted contracts. Read-only functions are deterministic for
captured inputs. Security mutation uncertainty is resolved from durable generation/operation identity,
not from in-memory state. Failures include invalid/expired grant, binding mismatch, unauthorized or
empty scope, missing overlap proof, live revocation/purge and security fail-closed.

## Required fixtures

Grant/binding/expiry/revocation matrix; scope-never-widens property; vendor-scope rejection; retrieval
and IDF predicate digest equality; inaccessible-corpus noninterference; overlap-proof drift; mutation
crash at every phase; revocation at every checkpoint; contaminated-leg whole discard; ordinary grant
validation performs zero redb writes.
