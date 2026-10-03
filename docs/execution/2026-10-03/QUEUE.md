# Execution queue after the Codex update

Prepared 2026-10-03 at the maintainer's request. This is a continuation plan, not a capability,
qualification or controller receipt. Read [RESTART.md](RESTART.md) first for the saved source and gates.

## Authority and reviewed updates

Search must operate standalone without Memory OS, Governor, assignment tickets or writer leases.
Architecture Part I, accepted ADR 0005/0006, current root instructions and package contracts apply.
Qdrant remains the only indexed substrate; redb holds technical control state.

- [PR #222](https://github.com/UnknownAlienHuman/eliot-search/pull/222), reviewed at
  `a669248737ac8982db70eb54b78ab17b8ddb199b`, is an **open documentation proposal**, based on
  `50b1e1b`. Its 19-file change adds an implementation-status matrix and expands the agent workflow.
  It does not establish runtime integration or qualification. Refresh its status snapshot and exact
  evidence references when integrating it; #220's complete status/evidence work remains outstanding.
- [PR #97](https://github.com/UnknownAlienHuman/eliot-search/pull/97) is historical planning based on
  the September 5 audit. Its old "main unchanged / all NOT_STARTED" wording is not current. Its
  packets may identify remaining product obligations, but neither its launch sequence nor ticket
  machinery blocks the current standalone work.
- Package source, compilation, strict Clippy, native/live qualification and public enablement are
  distinct facts. Keep each explicitly recorded. Full process suites follow implementation completion;
  use an immediate focused proof when a changed safety boundary requires it. Workflows stay manual-only.

## Dependency order

| Order | Work and actual owner | Prerequisite / completion boundary |
|---|---|---|
| 1 | Close the saved bridge-auth and Windows supervisor slices; retain the accepted daemon build repair. Owners: bridge, supervisor, daemon respectively. | Continue from the exact commits and findings in RESTART.md. Review native containment and auth before admitting them. Adapt the reclaimer's existing live fixture to the final authenticated connection API. Check each changed package once, then run only the needed focused native/auth proof. No global Qdrant qualification claim from these proofs. |
| 2 | Resolve [#199](https://github.com/UnknownAlienHuman/eliot-search/issues/199) payload/eligibility alignment and [#205](https://github.com/UnknownAlienHuman/eliot-search/issues/205) exact epoch transport. Owners: contracts/domain and concrete bridge integration. | Pinned Qdrant range transport uses f64; rejecting a nonrepresentable bound is fail-closed behavior, not support for the normative signed-i64 domain. Obtain an explicit contract/transport decision. Do not invent an epoch ceiling or silently switch provider/artifact. |
| 3 | Integrate canonical point identity → projection planner → publication: [#207](https://github.com/UnknownAlienHuman/eliot-search/pull/207) → [#209](https://github.com/UnknownAlienHuman/eliot-search/pull/209) → [#210](https://github.com/UnknownAlienHuman/eliot-search/pull/210). Owners: their three packages; root dependency/lock work belongs to integration. | Port/rebase onto current main, preserve the exact S11 identity and S9.5 payload, and reconcile private pinned BLAKE3 plus Cargo.lock ([#206](https://github.com/UnknownAlienHuman/eliot-search/issues/206), [#208](https://github.com/UnknownAlienHuman/eliot-search/issues/208)). Consume this as a coherent stack with bridge/daemon callers. Existing Linux package checks do not establish locked Windows or live acceptance. |
| 4 | Reconcile the real bridge with the canonical stack. Owner: search-qdrant-bridge. | [#200](https://github.com/UnknownAlienHuman/eliot-search/pull/200) and [#202](https://github.com/UnknownAlienHuman/eliot-search/pull/202) are competing revisions, not sequential merges. Preserve the current auth slice addressing [#201](https://github.com/UnknownAlienHuman/eliot-search/issues/201); select/port the required schema, route-generation, collision and scoped-IDF behavior. Do not mix legacy main identities/payloads with aligned producers. A new accepted collection generation/rebuild is required. |
| 5 | Implement package-owned Qdrant configuration and daemon composition. Owners: supervisor for qdrant_process, bridge for qdrant_data, daemon for composition; OS-secrets owns lease resolution. | Export descriptors, semantic validation, digests/change behavior and typed runtime settings from the actual owners. Resolve the single configured port versus distinct HTTP/gRPC ports, short private storage-root derivation and duplicate secret-reference fields. Register both sections, obtain the real purpose-bound lease, start/stop the owned child, and route production operations through the real bridge. No in-memory production fallback. |
| 6 | Finish canonical source/control/preparation and sparse encoding, then publish/retrieve validated evidence. Owners: source/revision/materializer/unitizer/lexical, control-redb, projection/publication, retrieval and validator/projector; daemon composes. | Confirm one durable owner and authoritative source/revision state, immutable preparation and exact manifests. Preserve access/currentness before retrieval, IDF and counts. Require real source → durable state → Qdrant → exact source readback, including restart, deny, unknown-outcome and rebuild. Complete W3 mandatory probes before advertising indexed capability. |
| 7 | Finish supported public v1 recipes. Owners: recipe/query packages, provider protocol, handles, daemon and CLI. | Execute supported exact/inspect/compare/provenance paths through the same state and authorization. Frozen-denominator verification and live handle reauthorization remain mandatory. Keep the eleven v1 recipes unchanged. |
| 8 | [#218](https://github.com/UnknownAlienHuman/eliot-search/issues/218): corpus and repository-portfolio management. Owners: contracts, source registry/reconcile, control state, daemon and CLI. | Fresh-install registration of roots/fleets, immutable portfolios, lineage/currentness and per-membership gaps/readiness. Public use must not require manual internal IDs or hand-authored control metadata. |
| 9 | [#213](https://github.com/UnknownAlienHuman/eliot-search/issues/213): versioned free-text retrieval and scope orientation. Owners: contracts/domain/query planning and the existing encoding/retrieval/validation/public-edge owners. | Co-design with #218's stable scope model. Add distinct bounded operations with request-local query text, grounded reading handles, explicit coverage and suggested exact checks. Do not overload find_text@1 or expose raw Qdrant queries. |
| 10 | [#221](https://github.com/UnknownAlienHuman/eliot-search/issues/221): deterministic lexical/structural ranking profile. Owners: lexical, projection, planner and retrieval. | Bind one profile to exact artifacts/configuration/fixtures. Verify golden vectors, deterministic fusion/diversity/lineage collapse, deny noninterference and real Qdrant/source readback. Free-text/orientation remains unavailable until its required profile evidence is accepted. |
| 11 | [#219](https://github.com/UnknownAlienHuman/eliot-search/issues/219): generic agent tool adapter. Semantic owner: provider protocol; assign the leaf adapter's package boundary explicitly. | Reuse the standalone management/recipe/grant/handle contract from #218/#213/#221. No direct Qdrant/redb/CAS access, controller authority or default network listener. Have Luna study an unfamiliar local repository and compare compact Search-guided reads with exact source checks; preserve actual results and limitations. |
| 12 | [#215](https://github.com/UnknownAlienHuman/eliot-search/issues/215) fleet-scale quality/resource qualification and [#216](https://github.com/UnknownAlienHuman/eliot-search/issues/216) document profiles. Owners: integration and each enabled profile. | Measure real large multi-repository workloads and quality, then separately qualify document coordinates/citations and safe no-execute acquisition. Unsupported/optional profiles remain explicitly disabled. |
| 13 | Complete lifecycle/security/recovery and reproducible installed Windows baseline. Owners: native runtime/security/storage packages and packaging integration. | Require the release's complete restart/recovery/purge/retention/transport and installed baseline evidence. An artifact install or successful CRUD is not product completion. |

## Parallel documentation and cleanup lane

- [#220](https://github.com/UnknownAlienHuman/eliot-search/issues/220): extract byte-identical normative
  Part I, preserve/archive historical Part II, repair entry-point links and publish current statuses
  with exact source/check/qualification evidence. PR #222 is partial progress, not completion.
- [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214): remove residual generic
  controller tooling from Search. Preserve reusable work in its external project/refs. This is separate
  from the Qdrant product spine and is not a prerequisite imposed on standalone startup.
- [#211](https://github.com/UnknownAlienHuman/eliot-search/pull/211): review useful ownership splits
  separately. Its 29 kernel-root relocations conflict with the already compiled in-place path repair;
  importing both path strategies would reintroduce bad paths. Its head has no claimed successful Rust
  execution. Do not transplant the old divergent integration branch wholesale.
- Include the earlier planner proposal [#204](https://github.com/UnknownAlienHuman/eliot-search/pull/204)
  in the #209 producer review; keep one canonical planner owner rather than assuming two independent
  merge steps. Revisit [#184](https://github.com/UnknownAlienHuman/eliot-search/pull/184) ownership/
  optimization and [#185](https://github.com/UnknownAlienHuman/eliot-search/pull/185) full-product test
  task packets against the finished implementation, not as a reason to start a test loop now.
- Older packet-only task PRs remain obligation references. Do not merge their old snapshots over newer
  product source or mark a task finished merely because its packet exists.

## Operating limits for the resumed run

Keep one writer per overlapping package and use Luna Max agents with explicit file ownership. No new
feature dispatch occurs before the saved current slices are reviewed. Do not reset, prune or delete
dirty/user/external worktrees. Keep raw logs, machine paths and credentials outside committed docs;
record exact commands, source revisions, unavailable checks and evidence identities. Fetch before
delivery, push accepted changes to main without force and verify remote readback.
