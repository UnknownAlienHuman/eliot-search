# Audit coverage and documentation corrections

**Source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
**Documentation examined:** `36fe7f439e2aa271852620fd33dc424cc975a88e`, re-reviewed 2026-10-21.
**Status:** broad static review with open design/documentation gaps. The preceding claim
`STATIC AUDIT COMPLETE FOR REFACTOR PREPARATION` is withdrawn. No exhaustive or runtime claim is made.

This file records evidence and unresolved work. It is not implementation authority, an agent launch gate,
a new task queue or a requirement to read every audit before coding. The manifesto and implementation guide
remain the two project-wide starting documents.

## Coverage

| Area | Evidence available | Limit |
|---|---|---|
| Workspace/entrypoints | Cargo targets, feature topology and principal caller paths; every workspace member has a proposed disposition | Disposition is not proof that every line or removal consumer was checked. |
| Source/root/config/control | Source maps trace dropped bytes, replay/archive layers, root and configuration blockers | Native behavior and the private object/key implementation were not executed/read. |
| Preparation/identity/Qdrant | Source maps and donor seams trace duplicate work, unstable identities and disconnected bridge calls | Duplicate-occurrence matching, manifest lookup cost and live scoring/mutation behavior are not fully demonstrated. |
| Publication/view/GC | Recorded state ownership, crash-boundary reasoning and multi-view cleanup condition | Runtime fault behavior and bounded GC traversal cost remain unverified. |
| Query/protocol/CLI | Principal proof/handle/recipe stacks and serving callers were inspected | This re-review found contradictions in the replacement design itself. |
| Tests/tooling/history | Families inventoried and major consumers traced | No exhaustive fixture-by-fixture or all-document sweep. |
| Optional features/donors | Selected implementations/callers and mechanisms inspected | Not an exhaustive donor survey; optional does not mean a cancelled product goal. |
| Private #598 | Public issue contract read | Unpublished source/worktrees untouched; public old requirements conflict with the new baseline. |

## Defects found in the replacement documentation

| Finding at the examined head | Why it invites excess or broken code | Correction / owner |
|---|---|---|
| This map declared itself normative and made #598 the only legitimate pre-code dependency | Recreates an approval hierarchy and blocks unrelated implementation | Maps are descriptive. Coordinate actual overlapping object/key files; do not inherit #598's old six-domain/purge/restore programme. Root AGENTS and implementation corrected. |
| Package table summary disagreed with its own 52 rows | False certainty can drive mechanical package recreation/deletion | Rows recount to 17 keep, 11 merge, 16 delete, 6 optional, 2 conditional. Map corrected; counts are not targets. |
| CLI closes child on every terminal but exposes Continue/Expand locators | Locators die before another invocation can use them; invites durable-token workaround | Explicit one-shot versus live multi-request session. One-shot emits no reusable window ID. CLI/protocol/window maps corrected. |
| Sequential read→handler execution combined with advertised Cancel | Cancel cannot be read while the handler blocks the reader | One active data request plus an independently serviced bounded control reader; not a general scheduler. Protocol map corrected. |
| Checking deadline before Write was treated as a complete timeout mechanism | A blocked call can outlive its deadline; fake cancellation wrappers multiply | Source itself says it cannot interrupt blocked I/O. Actual transport cancellation/owned-child termination must supply the bound. |
| `serde_json::to_vec(...) under encoded ceiling` | The ceiling is not enforced during allocation | `to_writer` to a capped local buffer, then frame/write; input DTO bounds remain necessary. |
| 'Original deadline' applied ambiguously to future pages | Either every page expires with Search or TTL is renewed indefinitely | Each request has its own deadline; fixed window expiry caps Continue/Expand. |
| Advance a prepared slice after formatting fewer cards | Output truncation can silently skip unprocessed candidates | Track the handled prefix: emitted cards plus explicitly represented gaps, not the proposed slice length. |
| Successful flush called delivery | Local write success does not establish client receipt | No exactly-once claim. Lost/partial connection drops session windows; caller repeats Search. |
| Pseudocode types/API lists and optional candidates were repeated across documents | Agents can implement a separate abstraction for every noun and every future option | Names are illustrative; only actual callers justify code. Wire/window algorithms have one owning FUNCTIONS map; READMEs link to it. |

### Source and donor checks for the I/O corrections

The actual [output_deadline.rs](../../bins/eliot-searchd/src/public_runtime_service/kernel/output_deadline.rs)
explicitly states it does not interrupt an already-blocked writer. This is a source fact, not a reproduced
runtime hang.

Official documentation checked 2026-10-21: Serde JSON 1.0.151 `to_writer` accepts an `io::Write` sink;
Tokio 1.53.1 `spawn_blocking` cannot abort a started closure. Current Tokio stdin documentation also describes
an uncancellable blocking read. References and the chosen application limits are in the
[protocol map](../../crates/search-provider-protocol/FUNCTIONS.md). This does not qualify a Windows pipe.

## What remains open

- Cross-check all remaining local instructions/active issue bodies against the changed session and
  implementation rules. Only affected issues are reconciled in this pass; no issue-number range implies that
  every intervening issue was checked.
- Specify bounded manifest point lookup and ambiguous occurrence matching before those operations are coded.
  Neither 'stable' nor 'paged' is an algorithm.
- Check GC traversal/locking cost and object-root completeness against the implemented producer/reference path.
- Reconcile the public #598 handoff with the simplified storage boundary without touching private work or
  reintroducing its old requirements.
- Separate buildable source, demonstrated behavior, measurements and proposed design in remaining historical
  claims. Native/live-Qdrant/crash/performance runs are still absent.

These are scoped follow-ups, not a new mandatory audit wave. Independent implementation need not wait for
unrelated checks. Never claim that assigning a disposition proves all documentation defects were found.

## Document roles

[Architecture](../architecture/README.md): purpose and constraints.
[Implementation](../implementation/README.md): selected mechanisms and receiving owners.
Local FUNCTIONS maps: concrete source seams and the single definition of an operation's behavior.
READMEs/AGENTS/issues: concise routing and change scope, not copies of the algorithm.

The [workspace map](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md),
[source findings](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) and
[donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) are supporting evidence. Historical 8.4/wave,
swarm, qualification and package-status material cannot direct new implementation. Preserve useful original
requirements as goals or explicit gaps instead of silently deleting them with their old mechanism.
