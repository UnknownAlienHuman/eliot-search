# Audit and implementation map

Start with the [Gemini audit verification and refactoring cards](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).
Its source baseline is c8e38b6fc9829f1cd38070773b1b3e5b21de18ad over PR #609's 239c5723.
That is a dated verification point, not a branch to reset to or a product-readiness claim.

## Authority and current work

- [Architecture Part I](../architecture/README.md), accepted ADRs and package contracts define semantics.
- [Root instructions](../../AGENTS.md) and [execution protocol](../product/EXECUTION_PROTOCOL.md) govern
  bounded work under the current maintainer request.
- [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), the current manager handoff,
  actual active issue/PR and merged source define the live queue. Preserve #598's active work/WIP.
- [PROJECT_COMPLETION](../product/PROJECT_COMPLETION.md) is an obligation graph. The dated launch gate,
  Wave-2 packet/matrix and earlier audit heads are historical snapshots.
- Audits identify evidence and candidate repairs. They do not activate features, authorize a broad
  multi-owner rewrite, or require completion of every finding before a safe source slice can merge.

## Do not repeat delivered work

| Existing work | Verified distinction |
|---|---|
| #237/#319, #258/#323 | Shared canonical hashing and indexed contracts exist. Reuse their actual APIs. |
| #256/#338, #257/#342 | Canonical S11 and verified UnitSet exist. Legacy identity removal remains #329. |
| #259/#519 | Planner S11/UnitSet algorithm and lookup repair are present; actual owner input integration remains. |
| #260 / PR #605 | Canonical publication admission is shared by submit/restore; actual effects remain #261/#263. |
| PR #608 | Lexical execution profile/budget work; not daemon/Qdrant integration. |
| PR #609 | Static pinned-artifact admission; not operational/live or installed qualification. |
| #262 | Bridge contract/route adoption remains; its old #256/#258 blockers are already delivered. |
| #598 | Current retained-source/reopen consumer, not completed by any of the above. |

## Historical evidence

The [10/09 master audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md),
[completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md),
[launch snapshot](AGENT_LAUNCH_GATE_2026-10-09.md) and
[package exception matrix](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md) retain useful paths and
obligations. Their “current”, “next” and BLOCKED statements require revalidation against current source.

One source writer, concrete owner/consumer changes, deletion of the selected replaced path, minimal
production check/Clippy and final independent review remain the implementation method. Test/native/
Qdrant/fault/scale qualification follows the complete integrated implementation. No ledger, document,
issue label, mock or compile establishes a product PASS.
