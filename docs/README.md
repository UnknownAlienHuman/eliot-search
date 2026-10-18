# Documentation map

ELIOT Search is a standalone analysis/retrieval framework. Qdrant owns indexed retrieval; Search owns
source truth, preparation, access, publication, exact verification and bounded agent navigation.
[ADR 0005](adr/0005-standalone-search-product-and-controller-boundary.md) defines the controller boundary;
[ADR 0006](adr/0006-agent-analysis-framework-product-scope.md) defines product scope.

## Read for the task at hand

| Need | Read |
|---|---|
| Begin a source change | [Root instructions](../AGENTS.md), [execution protocol](product/EXECUTION_PROTOCOL.md), exact current issue and nearest package `AGENTS.md`/`FUNCTIONS.md`. |
| Resolve product semantics | [Architecture entrypoint](architecture/README.md), its explicit baseline decisions, relevant [ADRs](adr/) and actual owning code/contract. |
| Reduce or replace code | [Verified audit and reduction map](audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md), [reuse index](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md), actual callers. |
| Find remaining outcomes | [Product completion map](product/PROJECT_COMPLETION.md); current handoff/#352 determine scheduling. |
| Understand agent experience | [Agent analysis framework](product/AGENT_ANALYSIS_FRAMEWORK.md). |
| Change wire/config/currentness | [Contracts](contracts/p00/), [configuration](config/), [current workspace](current/), [client](client/). |
| Qualify shipped behavior | [Evaluation](evaluation/), [optional profiles](optional/), repository-root [qualification inputs](../qualification/). |
| Recover historical rationale | [Audit index](audit/README.md), `handoff/`, `execution/` and frozen Git history; read only the relevant record. |

The current architecture replaces its listed 8.4 baseline obligations; older contracts cannot restore
them. Current compiled source establishes what exists, not qualification. Maintainer instructions and the actual issue/PR govern scope. Part II,
`swarm/**`, old launch packets and dated source maps are history, not a permission system or queue.
Do not require the full master and every historical packet for each change.

Maintain one owner for each rule. Update an existing contract/issue instead of appending another plan.
A dated observation must identify its base; current-state corrections belong in the current entrypoint.
Old source/feature status and copied dependencies must be checked before assigning work.

## Qualification boundary

Configured, operational and release-qualified are different states. Runtime checks observe current
process/schema/source state; test reports are not startup credentials. Native/Qdrant/recovery/scale/
installed evidence is required for the capability/profile being shipped. Source presence or compilation
does not supply it.
