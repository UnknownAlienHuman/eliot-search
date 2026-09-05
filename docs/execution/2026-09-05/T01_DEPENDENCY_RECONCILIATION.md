# T01 — locked dependencies and remaining acceptance boundary

Source: `8ef226d8dca4368e2fe83c37c870f56190b2c168`. This extends the earlier T01 inventory; it does not accept T01 or release T02.

## Complete checked-in lockfile inventory

`swarm/reconciliation/2026-09-05-locked-graph.json` contains every workspace dependency row from the exact lockfile: **47 workspace packages, 30 external package versions and 208 internal directed edges**. All dependency references resolve, the locked graph has no cycle, and workspace names match the root member paths. The input copies were checked against the Git blob IDs, not trusted after manual transcription:

| Input | Git blob |
|---|---|
| `Cargo.toml` | `d6977c6a9d3e2369d44b094103dbab737855ea07` |
| `Cargo.lock` | `098e3b3d7d4f59ed013ca52fe2d4ba25ca689f88` |
| daemon manifest | `97cdd44b1711574adacbf89fbde147ca6ab89ee2` |
| CLI manifest | `018dcaab2ba79944c372e3b96ef6748eee522343` |

The daemon package's locked transitive closure contains 38 other workspace packages and 15 external package versions. The CLI closure contains five other workspace packages and no external dependency. These are **lockfile closures, not the selected Windows/default-feature build or executed call graph**. Target-specific and optional records in Cargo.lock can overstate what a selected binary builds.

The inspected daemon manifest directly declares 12 nonoptional workspace dependencies plus `zeroize`, and 26 optional workspace dependencies. Materializer, unitizer and exact mechanics are mandatory in this manifest even though their full normative capabilities belong to later waves. A registry must not equate a wave number with Cargo feature activation or successful runtime composition.

`search-source-reconcile -> search-source-admission` remains explicitly classified as a dev dependency from its package manifest. Other edge kinds are not guessed from the lockfile. The earlier corrected `search-result-projector -> search-access` normal edge remains present.

## Two extra packages: disposition for review

Neither `search-revision-crypto` nor `search-os-secrets-windows` has an incoming edge anywhere in the checked-in locked graph. They are not dependencies of the primary daemon in that graph. This is stronger than a failed text search, but still does not exclude raw `#[path]`/`include!` reuse in uninspected source.

**Proposed disposition: retain their source/tests as unaccepted adapter candidates; do not add production dependencies or count them as qualified owners merely to make 45 equal 47.** T14 owns selection/migration of the revision crypto adapter; T18 owns the native secret adapter. Selection requires full reverse-use inventory, one existing capability owner, exact dependency pins, envelope compatibility and native tests. No code is deleted or silently substituted in this increment.

The two candidates must be represented explicitly in the final reconciled registry or removed from its active workspace through an independently reviewed isolation change. The 45-package registry is not declared fully reconciled while this remains unresolved.

## Concrete next change

[T02 target-isolation slice](T02_TARGET_ISOLATION.md) now names an exact two-manifest candidate patch. It separates eight experimental launch targets without removing their source or tests and without adding a feature that conceals them from `--all-targets`. This is not the complete capability extraction map and is **not applied to product manifests by T01**.

[Contract #48 reconciliation](../../handoff/CONTRACT_48_RECONCILIATION.md) distinguishes already specified primitive rules and existing implementations from the still missing named registry entries. It proposes the exact review surface without creating an accepted contract receipt.

## Remaining before acceptance

Native `cargo metadata` with dependency kinds/target/features, complete expanded module inventory, every package's handwritten source/test counts, the complete measured capability extraction map, accepted adapter disposition and independent contract/review evidence remain required. Counts of uninspected source are unknown, not zero. No compiler, Windows or live-Qdrant pass is issued here.
