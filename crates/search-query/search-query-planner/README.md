# search-query-planner — remove the proof protocol

The query owner already has one pinned view, resolved scope, typed request, constructed handlers and finite
request context. It selects direct operations from those values; it does not need a public planning authority.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [query cutover](../README.md) and local
[source map](FUNCTIONS.md).

## Revision-specific evidence

At the established audit revision, `src/lib.rs` measured 104,042 bytes and defined public capability, leg,
budget, grant and compiled-plan structures around eleven V1 recipes. Repository search found
`compile_canonical_plan` in package tests/support rather than normal daemon serving, while the daemon contained
another recipe/authority path. These are historical measurements and caller findings, not a permanent current
branch census.

Immediately before deletion or inlining, search current manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, feature/target wiring, tests and workflows. Name every surviving consumer and change it in the
same cut. Do not preserve two planning systems by strengthening hashes or wrapping one with another.

## Target

```text
TypedQueryRequest
+ PublishedViewGuard
+ ResolvedScope
+ RequestContext
+ actual handler references
→ direct operation(s)
```

Baseline dispatch:

```text
Search   → real Qdrant/native lexical handler
Exact    → bounded retained-source scan
Continue → QueryWindow page
Expand   → selected QueryWindow item neighborhood
```

No plan is created merely to call one service. Optional operations remain unsupported until their concrete
handler and evidence producer exist.

A small private operation list is justified only for real incompatible routes/profiles. It may group compatible
sources into one retrieval/IDF population, split incompatible physical routes, assign finite nomination ceilings
inside the existing absolute deadline/counters, preserve deterministic internal order and reject unsupported
modes before work. Qdrant-compatible branches should use native grouping/fusion rather than public leg objects.

## Remove after caller cutover

Delete capability tables, caller-created task/leg/compiled plans, plan IDs/fingerprints, grants, dependency and
overlap proofs, V1 recipe normalization, supplied drift observations, relative per-leg deadline resets and
fixtures that only seal/revalidate those records. Do not create a V3 plan, internal grant, synthetic dependency
digest or another registry.

Move any surviving route/profile grouping and checked limit allocation into query composition unless it remains
a substantial independent algorithm; otherwise delete the crate.

## Completion

The local `Search` handler reaches a constructed retrieval runtime without a capability, grant, recipe,
dependency or compiled plan. After code, run minimal Rust 1.98 check/Clippy for changed query/daemon/protocol
consumers and focused scope/route/limit/unsupported-mode cases. Planner fixture hashes and proof-shape tests are
removed with their APIs.
