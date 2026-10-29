# search-access — current API and removal boundary

## Current API

| Source/API | Actual behavior |
|---|---|
| `validate_grant` | Compares caller claims with caller validation booleans/time/generation and produces `ValidatedGrant`. |
| `intersect_scope` | Intersects requested memberships with a supplied grant set and supplied access snapshot. Useful non-widening selection is mixed with grant/snapshot authority. |
| `compile_base_eligibility` | Copies one membership/route plus security/shadow/purge generations and derives a custom predicate digest. |
| `compile_safe_legs` | Emits one leg per membership unless supplied `OverlapFreeRouteProof` allows grouping. The indexed executor then rejects grouped multi-plan legs. |
| `compile_pre_retrieval` | Sequences grant, recipe/modality/budget, scope, leg and live-state checks. |
| `recheck_live_access` / checkpoint permits | Compare supplied request/live structures at many phases; they do not read an owner. |
| `SecurityMutationBarrier` | Coordinates a durable restriction, live deny publication and dependent receipts through injected effects. It is not connected to ordinary local view selection. |

Gross source inventory is about 52 KiB (`lib.rs` 34,641 bytes; `barrier.rs` 17,500 bytes).
Production query/continuation/expansion calls still use `LiveExpansionBarrier::clean()` rather than an
owner-issued checkpoint.

## Replacement

```text
resolve_scope(requested, published_view, optional_restricted_session)
    -> ResolvedScope
```

`PublishedView` supplies exact current memberships, route/generation/profile and manifest inventory.
The resolver:

1. chooses configured/default or explicit source IDs;
2. rejects any ID outside the view or restricted session;
3. preserves deterministic order;
4. returns one union filter / projection-membership set for compatible Qdrant retrieval and IDF;
5. exposes the same selected inventory to exact scan and evidence validation;
6. splits only incompatible routes/profiles.

A source update or local selection change creates a new request/view; no live security generation is
needed. Existing pinned local work may finish. Restricted session revocation closes its connection,
cancels work and expires windows; no generic durable influence graph is introduced.

## Cutover

Change together:

- daemon `access_composition` local admission;
- query planner/executor inputs;
- Qdrant bridge filter/eligibility input;
- exact inventory selection;
- candidate validation and query-window session binding;
- provider protocol restricted-client boundary.

Then remove grant/recipe fields from local contracts, per-member plans, overlap proof, predicate digest,
phase permits, `LiveExpansionBarrier`, security-mutation barrier and their fixtures. A small
`ResolvedScope` may live in query composition; preserve this crate only if it still isolates a real
restricted-client policy.

## Verification

- explicit source selection never widens and unknown IDs fail;
- compatible multi-source selection becomes one retrieval/IDF corpus;
- excluded sources affect neither candidates nor statistics;
- exact scan and indexed query use the same view population;
- source update gives new requests a new view while an old local guard remains coherent;
- restricted session cannot request outside its set and revocation cancels its windows;
- no caller-created clean/checkpoint/digest value authorizes output.
