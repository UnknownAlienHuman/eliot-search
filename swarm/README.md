# Swarm control files

The repository contains **45 one-writer packages**: 41 libraries and 4 binaries. Cargo membership,
function/module/stage packets and qualification templates describe boundaries; `launch-state.toml` alone
decides what may run now.

Foundation order:

```text
search-contracts
  ├─ search-domain
  └─ search-ports
```

## Machine registries

- `crates.toml` — package paths, direct dependencies, earliest waves, assignments and limits;
- `function-packets.toml` — exact primary function/contract packet and package-only write scope for all
  packages;
- `module-packets.toml` plus `modules/*.toml` — exact logical module set and public entry module for all
  45 packages;
- `coverage/manifest.toml` plus `coverage/*.toml` — integration-owned crosswalk from Architecture Part I
  to sections, capabilities, invariants, ports, types/schemas, recipes, reasons, tasks and delivery
  slices;
- `stages.toml` — exact W0–W10 package sets, shared current-stage context and gate/completion-receipt
  ordering;
- `stage-readsets.toml` — exact replacement context for the **23 package assignments** reused after their
  earliest wave;
- `gates.toml` — central G0–G6 evidence requirements;
- `launch-state.toml` — current authorization and advancement conditions;
- `orchestration.toml` — issued ticket/lease/submission/review/handoff state machine and record layouts.

These registries are complementary:

```text
crates.toml
  tells the orchestrator what the package is and what it depends on

function-packets.toml
  tells the orchestrator which base contract and write scope the package owns

module-packets.toml
  tells the orchestrator which package-local modules own implementation state and the public entry point

coverage/manifest.toml
  proves that every normative architecture obligation has a concrete package/module owner

stages.toml
  tells the orchestrator why the package is active at the current stage and which shared files apply

stage-readsets.toml
  replaces earlier stage documents with accepted public handoffs when a package returns later

launch-state.toml
  tells the orchestrator whether an issued ticket may be considered for readiness

orchestration.toml
  tells the orchestrator how an issued ticket becomes a lease, submission, review and handoff
```

Presence in any registry is never implementation authorization by itself. The coverage registry is a
derivative audit/merge guard and cannot override Architecture Part I or an accepted public digest.

## Drafts versus issued records

P00 pre-issuance preparation is intentionally split:

```text
swarm/ticket-drafts/     non-claimable assignment boundaries
swarm/context-drafts/    source lists and registry selectors to materialize at an exact base commit

swarm/tickets/           issued immutable tickets only
swarm/context-manifests/ materialized immutable one-artifact writer contexts
swarm/leases/            active/historical writer leases
swarm/submissions/       package implementation submissions
swarm/reviews/           independent package review receipts
swarm/handoffs/          accepted append-only package/API handoffs
```

A draft has status `DRAFT_ONLY_NOT_ISSUED` or `UNMATERIALIZED_DRAFT`. It cannot enter the orchestration
state machine, authorize work, create a lease or be acknowledged by a writer.

At issuance, the integration owner selects an exact base commit, materializes the declared files and
package/function/module/stage registry fragments into one immutable writer-visible context artifact,
assigns distinct writer/reviewer identities and creates new ticket/context/lease records. The draft
itself is never copied verbatim into the issued-ticket directory.

The current P00 draft inventory is:

```text
search-contracts  AUTHORIZED launch class, but ticket not issued
search-domain     CONDITIONAL; accepted contracts handoff required
search-ports      CONDITIONAL; accepted contracts handoff required
```

Current claimable-record counts remain zero.

## Agent and integration files

- `assignments/` — one capability assignment per package;
- `modules/` — one exact package-local logical module packet per package, grouped by earliest wave;
- `coverage/` — exhaustive architecture ownership crosswalks;
- `ASSIGNMENT_PROTOCOL.md` — ticket, context, implementation and handoff procedure;
- `ASSIGNMENT_TICKET_TEMPLATE.md` — issued ticket template;
- `WRITER_LEASE_TEMPLATE.md` — package-local writer lease template;
- `CONTEXT_MANIFEST_TEMPLATE.md` — issuance-time context materialization manifest;
- `SUBMISSION_TEMPLATE.md` — package implementation submission record;
- `REVIEW_RECEIPT_TEMPLATE.md` — independent review record;
- `INTEGRATION_OWNER.md` — root/cross-package owner;
- `CONTRACT_CHANGE_TEMPLATE.md` — missing/changed contract, port or ownership request;
- `PACKAGE_HANDOFF_TEMPLATE.md` — completion receipt;
- `REVIEW_CHECKLIST.md` — package and integration review;
- `../docs/handoff/P00_DRAFT_CONTROL_PLANE.md` — P00 draft and issuance semantics;
- `../docs/handoff/SWARM_STAGE_READSETS.md` — exact stage-context assembly and examples;
- `../docs/handoff/ARCHITECTURE_COVERAGE_AUDIT.md` — exhaustive ownership audit and corrections.

## Context rule

An earliest-wave package receives its package/function/module/stage entries, assignment/base contract and
current stage shared read set. A package reused later receives one replacement override. The previous
stage packet and dependency implementation internals are not replayed; exact accepted public handoff
receipts replace them.

At issuance, repository source files and exact registry fragments are recorded with per-source SHA-256
and combined into one immutable context artifact. The writer receives that artifact plus bounded accepted
handoff/fixture references, not broad repository access.

`eliot-searchd` is progressively re-ticketed after W1 for source, index, query, currentness, proof,
lifecycle, generic-edge and optional composition. These are sequential one-writer package assignments,
not concurrent daemon agents.

Other explicit reentries include:

```text
eliot-search          W8 standalone client delta
search-eval           W9 Product Pulse, then W10 candidate-specific evaluation
search-publication    W7 lifecycle, then W10 scale migration
```

Static context is capped at sixteen files before materialization, except the exact manifest-closed P00
`search-contracts` pack. Ticket-added handoff and fixture references are separately bounded. Architecture
access remains exception-only after a concrete contract challenge.

## Architecture coverage rule

The machine crosswalk closes:

```text
45 package assignments
42 package-local function sources plus 3 foundation sources
479 package-local logical modules
S0-S39 architecture sections
C00-C30 capability cells
INV-01..INV-30 invariants
23 shared ports and exact methods
20 configuration sections
217 unique P00 named type/schema/registry symbols
11 public recipes
51 public/protocol/contract reason codes
P00-P18 delivery slices
```

A missing owner, invalid package/module ref, orphan assignment/function file, floating port owner,
unregistered named schema, uncovered package or delivery slice without exit evidence blocks merge and
ticket issuance. Static coverage does not mean implementation or acceptance.

## Current state

```text
current stage/wave:       P00 / W0
authorized:               search-contracts
conditional:              search-domain, search-ports
later stages:             BLOCKED
stage assignments:        68
later-stage overrides:    23
unique reused packages:   13
P00 ticket drafts:         3
P00 context drafts:        3
materialized contexts:     0
issued tickets:            0
active writer leases:      0
submissions/reviews:       0 / 0
accepted package handoffs: 0
runtime evidence:          absent
```

The orchestrator verifies all registry/context/ticket digests and accepted dependency/prior-stage
handoffs, creates an isolated package worktree, rejects out-of-scope writes and merges accepted packages
in dependency/stage order.
