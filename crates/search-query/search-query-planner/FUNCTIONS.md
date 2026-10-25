# search-query-planner — source audit and deletion map

The [query cutover](../README.md) owns the connected operation. This file maps inspected planner source to the
small amount of selection logic that may survive.

## Evidence boundary

At the established audit revision, the package was one 104,042-byte production file. The audit found the full
plan compiler in package tests/support rather than normal daemon serving, while the daemon carried another
recipe/capability/grant authority. This is immutable historical evidence, not current caller truth.

Before a code cut, repeat the exact current search across manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, features, targets, tests and workflows. Record all surviving consumers and move them in the
same change before deleting APIs or the crate.

## Inspected API behavior

| Source/API | Actual behavior at the inspected revision |
|---|---|
| public capability/budget/leg/plan structures | Caller-created values were later treated as accepted execution authority. |
| recipe normalization/support tables | Validated eleven frozen V1 variants and advertised static legs; did not select a constructed handler. |
| compiled-plan path | Consumed caller-created task plan, grant, scope/capability/safe-leg proofs and returned another record; created no Qdrant/source operation. |
| fingerprints/dependency/drift | Canonicalized supplied records and observations; read no current owner. |
| relative leg budgets | Divided durations/work across legs and encouraged nested timeout reset. |

Do not choose one disconnected planning framework and wrap the other.

## Replacement selection

The query owner receives one typed request, `PublishedViewGuard`, non-widening `ResolvedScope`, one
`RequestContext` and actual constructed handlers. Baseline dispatch is direct:

```text
Search   → lexical/Qdrant handler
Exact    → retained-source scan
Continue → QueryWindow page
Expand   → selected-item neighborhood
```

Optional operations appear only with real handlers. Unsupported modes fail before backend/source work.

For actual multiple physical routes/profiles, a private helper may produce a finite ordered list. Compatible
sources share one retrieval/IDF population; split only incompatible routes; every branch uses the same absolute
deadline/cancellation and a finite nomination ceiling. Deterministic order is internal behavior, not a signed
plan identity.

## Delete after caller cutover

Remove V1 recipe normalization/support tables, public capability/task/leg/compiled plan types, grants, overlap
and dependency proofs, supplied drift observations, relative per-leg deadline allocation and fixtures that only
seal/revalidate those records. Remove matching contract/provider/daemon authority types with their last callers.

Move any surviving grouping and checked-limit helpers to query composition. Delete this crate if no substantial
independent algorithm remains.

## Verification after code

Run minimal Rust 1.98 check/Clippy for changed production consumers. Through the connected application verify
that explicit/empty/unknown scope never widens, compatible sources share the selected population, incompatible
routes split deterministically, all work uses one absolute context, and unsupported optional modes perform no
backend/source I/O.
