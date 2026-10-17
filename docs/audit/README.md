# ELIOT Search: audit and implementation entrypoint

**Primary remediation mandate (2026-10-17):** [ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).
**Current merged `origin/main` base:** `239c5723` (PR #609).
**Live programme queue:** issue #352 + the 4-phase remediation plan in [ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).
**Product/release:** not qualified (`eliot-searchd` spine remains unwired to Qdrant by default; fake FNV-1a digests and legacy 16-byte S11 types still exist on `main`).

Always create new writer branches from actual `origin/main`, never from a dated local audit SHA.

## Authority by purpose

1. [Root instructions](../../AGENTS.md), [Architecture Part I](../architecture/README.md), accepted ADRs and nearest package instructions define product semantics and ownership.
2. [2026-10-17 Architectural & Code Audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) defines the mandatory remediation of fake cryptography (`FNV-1a` in `Blake3Digest32`, hand-rolled `SHA-256`, 16-byte FNV `PointIdentityDigest`), security bypasses (`path.parent()` tautological root containment, `[0xA5; 32]` vault receipts), illegal in-memory lexical index (`bins/eliot-searchd/src/lexical/`), and unwired `#[allow(dead_code)]` daemon modules.
3. [Bounded execution protocol](../product/EXECUTION_PROTOCOL.md) defines work-item classes, Definition of Ready, `SCOPE_FROZEN`, finding triage, review budgets, source gates and serial merge discipline.
4. [Programme issue #352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), [launch gate](AGENT_LAUNCH_GATE_2026-10-09.md), the exact active issue/PR and current source define the **live writer queue**.
5. [Whole-project completion map](../product/PROJECT_COMPLETION.md) is the long-range obligation/dependency graph. Its dated “next task”, audit SHA or local stage wording is not live scheduling authority.
6. [Completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md) and [Master technical audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) retain earlier unresolved technical evidence.

When the long-range map is stale, follow [ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md), #352, merged artifacts and the exact issue; update the map separately. Never repeat delivered work because an old row still looks active.

## Mandatory anti-slop rules for all agents

1. **No fake digests or hand-rolled crypto:** Do not write 4-lane FNV-1a (`0xcbf2_9ce4_8422_2325`), SplitMix64, or hand-rolled SHA-256 (`0x428a_2f98`). Use workspace `blake3` (with length-prefixed inputs or canonical CBOR) and `sha2`.
2. **Never add to `xtask/canonical-digest-ledger.json`:** Fix the fake digest/receipt in the source crate and delete its exemption entries.
3. **Cut over and delete legacy duplicates in the same slice:** Updating a crate plus its immediate reverse consumers to delete broken legacy types (`PointIdentityDigest([u8; 16])`, `bins/eliot-searchd/src/lexical/`, `src/snapshot/`, `src/sealed_*`, `src/main.rs`, `src/bin/eliot-search-sealed-*.rs`) is explicitly authorized and required.
4. **Wire the real daemon spine:** Do not add `#[allow(dead_code)]` composition facades in `bins/eliot-searchd`. Enable `full-baseline` by default and wire `entry.rs::main()` end-to-end through `redb` and `search-qdrant-bridge`.

## Delivered source checkpoints

| Task | PR | Actual delivery |
|---|---:|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest APIs and source-classification guard |
| #253 | #322 | Unicode 18 full-fold decision/goldens; not a tokenizer |
| #258 | #323 | Shared indexed payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata and package-status/dependency validation |
| #256 | #338 | Stateless S11 point identity submodule; legacy 16-byte FNV retirement in `lib.rs` still required (#329) |
| #339 | #340 | Windows Cargo path-prefix handling in Qdrant tooling |
| #327 | #341 | Rustls 0.23.45 security repair; live requalification remains later |
| #257 | #342 | Source-verified UnitSet v3; durable preparation integration remains #331 |

These are scoped source deliveries, not installed/native/Qdrant/full-product acceptance.

## One-manager merge train

One manager owns one writer worktree, commits, merges, dependency pins and `Cargo.lock`. Subagents perform bounded read/research/review assignments. One writer does not mean one giant PR.

```text
ready slice
→ SCOPE_FROZEN by second source commit
→ bounded subagent reports
→ manager implementation (cutting over & deleting legacy/fake code)
→ changed owners + immediate reverse-consumer check/Clippy
→ formal exact-final-SHA review
→ merge
→ publish new main SHA
→ create next writer branch from new main
```

No stacked source PRs. New findings are classified `B0`, `F1`, `F2`, `D` or `Q`; they do not automatically widen the active branch.

## Programme bindings

| Programme | Executable source owner |
|---|---|
| Fake crypto & S11 cutover | Phase 1 of `ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` (#329 + FNV-1a/SHA-256 removal) |
| Daemon legacy purge & security fixes | Phase 2 of `ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` |
| End-to-end Qdrant/redb spine wiring | Phase 3 of `ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` |
| Root admission | #266 bounded tranches; residuals #343/#345/#346/#347/#348 |
| Provider edge | #235.core then #332 native cutover |
| Configuration | #238 parser then #333 durable apply |
| Revision storage | #330, after #307–#309 secret/crypto owners |
| Preparation | #331, consuming delivered #257 |
| Qdrant process | #334 implementation, #310 role wiring, #119/#264/#215 acceptance |
| Git | #335 |
| Overlay/LSP | #336 core then optional leaf |
| Acceptance/release | #233/#234/#240/#242/#215, then #137/#140 human publication |

## Evidence boundary

Compilation is necessary but not qualification. Source delivery, integration, checked source, native qualification, installed qualification and release are distinct states. Preserve failures, partial results, unavailable cases and NOT_RUN outcomes.

No documentation, issue rewrite, source guard, exemption ledger (`xtask/canonical-digest-ledger.json`), signature, review badge or author comment creates a Cargo/Clippy/native/Qdrant/installed/scale/release PASS. Historical `swarm/**`, handoff packets, Architecture Part II and closed packet PRs are archaeology and obligation records only.
