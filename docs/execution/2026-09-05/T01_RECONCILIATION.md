# T01: current package and assignment reconciliation

Baseline: `a5abdf7ef0cb9d691000759494fd8829b2ba0b60`. Work branch: `plan/search-20260905-t01`, PR #98.
**IN_PROGRESS. This increment does not accept T01 or release T02.**

## Corrected

`swarm/crates.toml` omitted the normal dependency `search-result-projector -> search-access`, which is already present in the package Cargo manifest. This increment adds that one edge; no Rust source, Cargo manifest, lockfile, package scope or launch gate changes.

`search-source-reconcile -> search-source-admission` is different: it is a **dev-dependency**. It is recorded separately, not added as a normal runtime edge. Cargo.lock alone is insufficient to recover dependency kinds.

## Current inventory

| Observation | Result | Evidence boundary |
|---|---|---|
| Explicit root Cargo members | 47 | Root manifest bytes checked against Git blob identity |
| Registered packages | 45: 41 libraries, 4 binary packages | Registry bytes checked against Git blob identity |
| Unregistered members | `search-revision-crypto`, `search-os-secrets-windows` | Existing member manifests, not approved new owners |
| Direct primary module declarations | 23 plus one Windows test module | `entry.rs` declarations, **not** complete transitive reachability |
| Legacy/prototype isolation candidates | 8 targets | Two explicit snapshot targets and six `src/bin` candidates; no removal in this change |
| Open legacy issues mapped | 38 | All mapped to existing T01–T43 task PRs; none marked completed |

The machine inventory is [`swarm/reconciliation/2026-09-05.json`](../../../swarm/reconciliation/2026-09-05.json). Exact observed source blob identities are included so a later check does not silently use another baseline. Counts of binary **packages** do not count auto-discovered binary **targets**.

The two extra members must either receive one reviewed adapter owner and consistent package/function/module/stage entries, or be explicitly removed/extracted after inspecting their callers. Merely changing `45` to `47` would hide the missing ownership rather than repair it. Source presence also does not prove either helper is used by the primary runtime.

## Old assignments

[`LEGACY_ISSUES.md`](LEGACY_ISSUES.md) routes all 38 observed open issues. Important conflicts are named-pipe-only versus loopback IPC (#74/#90), unreviewed cipher replacement (#83), and replacing raw source coordinates with normalized text (#84). These need a single accepted contract/profile decision, not parallel implementations.

Existing implementation, historical merge, current executed tests and accepted handoff are separate facts. In particular, #24 concerns a pack-verifier review, not full contracts acceptance. #48 remains a contract-decision prerequisite; this increment does not approve its proposed shapes. Historical failed run #93 is not a current-head build result.

No issue is bulk-closed. Routing to an existing task prevents duplicate implementation requests; it does not prove those implementations complete.

## Verification and remaining T01 work

Authoring checks compare the before/after registry, exact source Git blob hashes, unique member paths, the complete legacy issue set, task references and direct-module aliases. Negative fixtures reject a missing corrected edge, duplicate package/issue, wrong member path and premature acceptance. These are static checks of this inventory, not repository Rust tests.

The attempted workspace check returned exit 127 because `cargo` is absent. No compiler, native Windows, Qdrant, independent review or full registry-parity PASS is claimed.

Before T01 can close, finish the full normal/dev/build/target/feature dependency graph from `cargo metadata`, per-package handwritten source/test line counts, transitive module reachability, all package/function/module/stage parity, #48/public-API evidence reconciliation and the exact T02 move map with bounded contexts. These remain explicit blockers in the machine inventory; no unknown measurement is recorded as zero.

T02 is not authorized by this document. Existing launch and handoff rules remain unchanged. Review this bounded correction without mistaking it for full swarm readiness.
