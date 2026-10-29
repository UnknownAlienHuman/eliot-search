# Working on search-domain

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), package [README](README.md) and
[source/caller audit](FUNCTIONS.md).

This package is a deletion target unless a small real multi-consumer algorithm survives the direct-owner
cutovers. Do not extend, version or replace its broad proof algebra.

Most exported assurance, coverage, currentness, fingerprint, eligibility, ordering and visibility operations
have no external production caller. `transition_publication` and source ownership v2 serve old publication/
registry systems already being removed. Move or delete them with those callers. Inline/move the small mutation
outcome rule into concrete effect owners unless two final owners truly share the same API.

Rules belong with actual state/effects:

- source compare-set in the source/control owner;
- pending/current/cleanup transitions in publication;
- scope intersection in query;
- complete-negative semantics in Exact;
- evidence assurance in manifest/object verification;
- ordering in Qdrant/native fusion or one real merge;
- readiness from actual constructed handlers;
- unknown outcomes from authoritative effect-specific readback.

Do not create `search-domain-v2`, generic `Decision`/reason/fingerprint wrappers, forwarding re-exports or
policy kernels for hypothetical features. Remove unused `search-domain` Cargo dependencies as callers move.

After code, run minimal Rust 1.98 compiler/Clippy for changed direct owners and actual consumers. Delete
package-only proof/golden/property tests with the APIs they protect; verify behavior through the owning
operations.
