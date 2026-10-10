# ELIOT Search: audit and implementation entrypoint

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.
**Live programme queue:** issue #352.
**Source programme:** #266; PR #344 closed unmerged/frozen at `6d698eafa573e7a8a5eb9dca6ce17ae7878c3bce`.
**Process correction:** #349 and #350 / PR #351.
**Product/release:** not qualified.

Do not reset an active manager worktree to a dated audit SHA. Resolve actual `main`, active PR head and exact owner before writing source.

## Authority by purpose

1. [Root instructions](../../AGENTS.md), [Architecture Part I](../architecture/README.md), accepted ADRs and nearest package instructions define product semantics and ownership.
2. [Bounded execution protocol](../product/EXECUTION_PROTOCOL.md) defines work-item classes, Definition of Ready, `SCOPE_FROZEN`, finding triage, review budgets, source gates and serial merge discipline.
3. [Programme issue #352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), [launch gate](AGENT_LAUNCH_GATE_2026-10-09.md), the exact active issue/PR and current source define the **live writer queue**.
4. [Whole-project completion map](../product/PROJECT_COMPLETION.md) is the long-range obligation/dependency graph. Its dated “next task”, audit SHA or local stage wording is not live scheduling authority.
5. [Completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md) and specialized audits retain unresolved technical evidence.

When the long-range map is stale, follow #352, the launch gate, merged artifacts and the exact issue; update the map separately. Never repeat delivered work because an old row still looks active.

## Delivered source checkpoints

| Task | PR | Actual delivery |
|---|---:|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest APIs and source-classification guard |
| #253 | #322 | Unicode 18 full-fold decision/goldens; not a tokenizer |
| #258 | #323 | Shared indexed payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata and package-status/dependency validation |
| #256 | #338 | Stateless S11 point identity; legacy retirement remains #329 |
| #339 | #340 | Windows Cargo path-prefix handling in Qdrant tooling |
| #327 | #341 | Rustls 0.23.45 security repair; live requalification remains later |
| #257 | #342 | Source-verified UnitSet v3; durable preparation integration remains #331 |

These are scoped source deliveries, not installed/native/Qdrant/full-product acceptance.

## Current active work

PR #344 preserves substantive #266 work as a frozen read-only donor. Merge source-free #351 first, then open only the first dependency-ready #354 child from that new `main` with extraction provenance. #355/#356/#358 are PROGRAM parents and cannot authorize broad source branches.

Separate follow-ups discovered during #266:

```text
#343 non-mutating redb inspection
#345 record-artifact unknown-outcome staging retention
#346 remaining all-target fixture compilation debt
#347 native original-object unlink/late cleanup
#348 original request to durable-effect reconciliation
```

They are not implicit blockers for every root-admission tranche. Each must demonstrate an exact `B0` compile/safety dependency or remain a follow-up with affected behavior unavailable/fail-closed.

After the minimum typed root-admission API and required immediate callers merge, `#235.core` may begin from the resulting `main`. It does not wait for complete #266 programme, harness, cleanup, redb inspection or release qualification.

## One-manager merge train

One manager owns one writer worktree, commits, merges, dependency pins and `Cargo.lock`. Five to ten subagents perform bounded read/research/review assignments. One writer does not mean one giant PR.

```text
ready slice
→ SCOPE_FROZEN by second source commit
→ bounded subagent reports
→ manager implementation
→ changed owners + immediate reverse-consumer check/Clippy
→ affected static guards + deferred integrated test inventory
→ formal exact-final-SHA review
→ merge
→ publish new main SHA
→ create next writer branch from new main
```

No stacked source PRs. New findings are classified `B0`, `F1`, `F2`, `D` or `Q`; they do not automatically widen the active branch.

PROCESS/DOCS are source-free. Source merges use minimum production check/strict Clippy/static guards; tests are specified with their owner and deferred to integrated qualification. Reuse existing broad debt rather than rebuilding harnesses per slice. Formal final-SHA review records no unresolved B0/P1 and explicit residual dispositions; self-approval is not required. After #351, three bounded source merges precede another process programme PR unless a verified B0 blocks coding.

## Programme bindings

| Programme | Executable source owner |
|---|---|
| Root admission | #266 bounded tranches; residuals #343/#345/#346/#347/#348 |
| Provider edge | #235.core then #332 native cutover |
| Configuration | #238 parser then #333 durable apply |
| Revision storage | #330, after #307–#309 secret/crypto owners |
| Preparation | #331, consuming delivered #257 |
| Qdrant process | #334 implementation, #310 role wiring, #119/#264/#215 acceptance |
| Git | #335 |
| Overlay/LSP | #336 core then optional leaf |
| Acceptance/release | #233/#234/#240/#242/#215, then #137/#140 human publication |

Historical programme/tracking/gate PR branches are not implementation bases. They are closed after current owner links are preserved; their discussions remain history.

## Technical findings and donors

[Master technical audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) and specialized audits retain unresolved findings. Older snapshots and candidate versions are evidence, not execution authority.

[Wave-2 donor register](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md) records prior decisions. Recheck exact versions, features and advisories when adopting or changing a dependency. Prefer the smallest mature donor that removes custom low-level code; donor types and authority stay behind ELIOT boundaries.

## Evidence boundary

Compilation is necessary but not qualification. Source delivery, integration, checked source, native qualification, installed qualification and release are distinct states. Preserve failures, partial results, unavailable cases and NOT_RUN outcomes.

No documentation, issue rewrite, source guard, signature, review badge or author comment creates a Cargo/Clippy/native/Qdrant/installed/scale/release PASS. Historical `swarm/**`, handoff packets, Architecture Part II and closed packet PRs are archaeology and obligation records only.
