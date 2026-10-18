# Wave 2 single-manager packet — historical pointer

The old packet used base `de07c2214097a596066d6afac308394d9b4bcd62` and repeated a
#351 → #354 → #235.core / Wave-2 train. It is no longer a current task list or launch gate.
[Preserved packet and its evidence](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/audit/WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md).

Use the existing records for their distinct purposes:

| Question | Current entrypoint |
|---|---|
| What may change, and how is it reviewed? | [Root instructions](../../AGENTS.md) and [Implementation](../implementation/README.md). |
| Which writer is active? | Current maintainer handoff, [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), actual issue/PR and merged source. Preserve #598 and its WIP. |
| Which product result remains? | [Implementation](../implementation/README.md). |
| Which duplicate can be removed? | [Verified source/reduction map](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md). |
| Which donor/code is already available? | [Reuse index](DONOR_VERIFICATION_REGISTER_2026-10-09.md) and [indexed donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md). |
| Where is historical path evidence? | [Exception matrix](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md) and [donor archive](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md), verified against actual callers when relevant. |

No packet issues an assignment, introduces another writer or requires repeating completed source work.
A package exception is an explicit owner-plus-immediate-consumer cutover, not permission for a broad
rewrite. A dated source head is never a reason to reset the active manager's worktree.
