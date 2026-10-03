# Authority and source-of-truth map

This map resolves conflicts between product architecture, package contracts, repository instructions and
historical development-planning material.

## Precedence

1. **Architecture Part I** — product behavior, security, state and runtime invariants.
2. **Accepted product ADRs** — implementation/package decisions that do not weaken Part I. ADR 0005 is
   authoritative for the standalone/controller boundary.
3. **Accepted public API/schema/port/configuration contracts** — immutable surfaces consumed by product
   packages and external adapters.
4. **Package ownership documentation** — nearest `AGENTS.md`, `FUNCTIONS.md`, Cargo manifest and package
   docs.
5. **Current maintainer request and issue/PR acceptance criteria** — exact work to perform within the
   preceding product contracts.
6. **Qualification packets and exact executed evidence** — whether a capability may be claimed or
   enabled.
7. **Planning and historical metadata** — `swarm/**`, `docs/handoff/**`, `docs/execution/**`, drafts and
   prior campaign records.

Planning/history material cannot override items 1–6, cannot authorize or block implementation and is
never a runtime input.

## Product ownership

| Responsibility | Owner |
|---|---|
| Source admission, revisions, preparation and retrieval projections | ELIOT Search |
| Qdrant process/schema/publication/retrieval | ELIOT Search |
| Query access fences, candidate validation, result projection and handles | ELIOT Search |
| Standalone daemon and CLI | ELIOT Search |
| Typed provider translation for ELIOT | `search-eliot-adapter` as a Search leaf adapter |
| Tasks, WorkScope, canonical task history, Context Compiler, Governor and finish | ELIOT Memory OS |
| Agent task/attempt/binding/operation/check orchestration prototype | ELIOT Swarm Controller |
| Native model loop and harness internals | The selected external harness/adapter |

Search may expose typed provider operations to ELIOT. That does not transfer task authority into Search
or make ELIOT/controller services prerequisites for standalone operation.

## Standalone rule

`eliot-searchd` and `eliot-search` must support installation, startup, ingestion, indexing, search,
rebuild and recovery without:

- ELIOT Memory OS;
- a Governor or General Manager;
- `eliot-swarm-controller`;
- assignment tickets or writer leases;
- approval/signature profiles;
- agent roles, mailboxes or schedulers.

Any configuration or code path that makes those dependencies mandatory violates ADR 0005.

## Repository-work authorization

An explicit maintainer request, issue or PR plus one non-overlapping branch/worktree authorizes normal
repository work. Package boundaries and dependency direction still apply.

No immutable assignment ticket, materialized context artifact, acknowledgement, lease, receipt,
signature profile or launch-state transition is required before editing or reviewing Search.

`swarm/launch-state.toml` is a legacy planning snapshot. Its package arrays and counters are not a lock,
permission system, product health state or acceptance record. Lower-level documents that call it the
"sole current authorization" are historical and non-authoritative.

## Product qualification authority

A product capability is accepted only by its exact executed product evidence:

- Qdrant qualification for indexed mode;
- provider/client transport and native security qualification;
- query/currentness/exact-proof qualification;
- real end-to-end product-spine evidence;
- installed release evidence.

Cargo membership, source presence, planning metadata, ticket state, schemas or mock/oracle execution do
not enable a capability.

## Conflict handling

- A Part I or ADR 0005 conflict stops the product change and is resolved at the product contract.
- A generic controller requirement inside Search is removed or moved to the controller/Memory OS
  repository; it is not completed locally.
- A package dependency or ownership mismatch blocks that code change, not unrelated package work.
- Missing executed qualification keeps the affected capability disabled; it does not block development
  of the implementation needed to obtain that qualification.
- A historical `swarm/**` rule that conflicts with current product work is ignored and corrected rather
  than implemented.
- ELIOT integration must remain an optional leaf adapter over the same standalone owners.

## Development metadata disposition

The repository may retain small static aids for package mapping, architecture coverage and non-overlap.
They are advisory. Do not extend them into a generic executable control plane.

The following belong outside ELIOT Search:

- ticket/lease/acknowledgement issuers;
- actor/approval/signature registries;
- task/attempt/operation/check databases;
- mailboxes, scheduler queues and manager routing;
- harness lifecycle and agent observability;
- review acceptance or finish authority.

Reusable implementations go to `UnknownAlienHuman/eliot-swarm-controller`; canonical task/memory
semantics go to `UnknownAlienHuman/eliot-memory-os`.

## Evidence rule

Evidence records must identify the exact product revision, commands, environment, inputs and outcomes.
Unknown or unavailable results remain explicit. Historical evidence about repository-control tooling is
not Search product evidence. Diagnostic Qdrant observations are retained only as unqualified inputs until
rerun through the accepted qualification packet.
