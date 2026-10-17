# Audit and reduction map

Start with the [verified Gemini audit and measured reduction work](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md)
and the [donor/reuse index](DONOR_VERIFICATION_REGISTER_2026-10-09.md). Both identify their source base,
confirmed behavior and limits; neither asserts product readiness or assigns a new writer.

## Current use

| Question | Record |
|---|---|
| Semantics | [Architecture Part I](../architecture/README.md), accepted ADRs and owning contracts. |
| Bounded implementation | [Root instructions](../../AGENTS.md), [execution protocol](../product/EXECUTION_PROTOCOL.md), current maintainer task. |
| Actual queue | Current handoff, [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352), exact active issue/PR and merged source. Preserve #598/WIP. |
| Delivered versus remaining | [Completion map](../product/PROJECT_COMPLETION.md) and [indexed donor disposition](INDEXED_SPINE_PORT_MAP_2026-10-09.md). Check actual callers before reusing a dated claim. |

## Historical evidence

The [10/09 master audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md),
[completion audit](PROJECT_COMPLETION_AUDIT_2026-10-09.md),
[launch snapshot](AGENT_LAUNCH_GATE_2026-10-09.md),
[package exception matrix](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md) and
[donor archive](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md) retain provenance, paths and obligations.
Their “current”, “next”, BLOCKED and old phase statements require source revalidation. The
[old manager packet](WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md) is a pointer, not another protocol.

Audits do not require repairing every finding before a coherent slice can merge. Test/native/Qdrant/
fault/scale qualification follows complete integration. A ledger, issue status, mock or compile is not a PASS.
