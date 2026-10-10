# ELIOT Search: audit and implementation entrypoint

**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.  
**Active source programme:** #266, draft PR #344.  
**Process correction:** #349 and #350.  
**Product/release:** not qualified.

Do not reset an active manager worktree to a dated audit SHA. Before writing source, resolve the actual current `main`, active PR head and exact issue ownership.

## Start here

1. [Bounded execution protocol](../product/EXECUTION_PROTOCOL.md): mandatory work-item classes, Definition of Ready, `SCOPE_FROZEN`, finding triage, PR review budget, source gates and merge discipline.
2. [Whole-project completion map](../product/PROJECT_COMPLETION.md): implementation-to-release DAG and source owners.
3. [Root instructions](../../AGENTS.md), [architecture](../architecture/README.md), accepted ADRs and nearest package instructions.
4. [Coordinator #97](https://github.com/UnknownAlienHuman/eliot-search/pull/97), exact active implementation issue and current source.
5. [Wave-2 packet](WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md) and [package exceptions](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md) only for still-applicable package/path details.
6. [Completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md) and specialized source audits for unresolved technical findings.

The execution protocol controls repository workflow. The completion map controls ordering. Architecture and package owners control product semantics. A programme issue may remain open while a coherent source slice merges.

## Current delivered source

| Task | PR | Result |
|---|---:|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest APIs and source-classification guard |
| #253 | #322 | Unicode 18 full-fold decision/goldens; not a production tokenizer |
| #258 | #323 | Shared indexed payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata and package-status validation |
| #256 | #338 | Stateless S11 point-identity profile; legacy retirement remains #329 |
| #339 | #340 | Equivalent Windows Cargo path-prefix handling in Qdrant boundary tooling |
| #327 | #341 | Rustls 0.23.45 security patch; live Qdrant requalification remains later |
| #257 | #342 | Source-verified UnitSet v3; durable preparation integration remains #331 |

These are scoped source deliveries. None is whole-product, installed, Qdrant or release acceptance.

## Current active work

PR #344 attempts #266 typed data-root admission and explicit initialization. It contains substantive code, but at its current published head it has exceeded the bounded-review thresholds and remains draft. Process issue #349 requires freezing or splitting it. The repository-wide rule is #350 and this protocol.

Current separate follow-ups discovered during #266 include:

```text
#343 non-mutating redb inspection
#345 record-artifact unknown-outcome staging retention
#346 remaining all-target fixture compilation debt
#347 native original-object unlink/late cleanup
#348 original request to durable-effect reconciliation
```

They are not implicit blockers for every root-admission source tranche. Each must demonstrate a `B0` dependency or remain a follow-up with the affected behavior fail-closed/unavailable.

After the minimum typed root-admission API and required immediate callers merge, `#235.core` may begin from the resulting `main`. It does not wait for complete #266 programme, harness, cleanup, redb-inspection or final qualification closure.

## One-manager execution

One manager owns one writer worktree, dependency pins, `Cargo.lock`, commits and merges. Five to ten subagents perform bounded read/research/review assignments. The single-writer model does not justify a large PR.

```text
ready slice
→ scope freeze
→ bounded subagent reports
→ manager implementation
→ changed owners + immediate reverse-consumer check/Clippy
→ focused causal fixtures
→ formal review at exact final SHA
→ merge
→ new main SHA
```

New findings are classified `B0`, `F1`, `F2`, `D` or `Q`. They do not automatically widen the active branch.

## Programme bindings

| Programme | Executable source owner |
|---|---|
| Qdrant process / PR #120 | #334 implementation, #310 role wiring, #119/#264 acceptance |
| Git / PR #129 | #335 |
| Overlay/LSP / PR #130 | #336 core then optional leaf |
| Revision / PR #111 | #330, with #307–#309 secret/crypto prerequisites |
| Preparation / PR #113 | #331, consuming delivered #257 |
| Config / PR #109 | #238 parser then #333 durable apply |
| Provider edge / PR #116 | #235.core then #332 native cutover |
| Acceptance / PR #137 | #233/#234/#240/#215, only after implementation closure |
| Release / PR #140 | #242 candidate, #215 run, #234/#137 review, human publication |

Historical programme/tracking/gate PR branches are not implementation bases. Under the current protocol, non-mergeable PRs should be closed after their obligations are preserved in current issues/docs.

## Technical findings and donors

[Master technical audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) and specialized audits retain unresolved source findings. Older snapshots and candidate versions are evidence, not current execution authority.

[Wave-2 donor register](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md) records prior decisions. Recheck exact versions, features and advisories when actually adopting or changing a dependency. Prefer the smallest mature donor that removes custom low-level code; donor types and authority remain behind ELIOT boundaries.

## Evidence boundary

Compilation is necessary but not qualification. Source delivery, integration, checked source, native qualification, installed qualification and release are distinct states. Preserve failures, partial results, unavailable cases and NOT_RUN outcomes.

No documentation, issue rewrite, source guard, signature, review badge or author comment creates a Cargo/Clippy/native/Qdrant/installed/scale/release PASS. Historical `swarm/**`, handoff packets, Architecture Part II and non-mergeable packet PRs are archaeology and obligation records only.
