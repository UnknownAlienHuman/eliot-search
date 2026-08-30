# Swarm launch and stage-specific read-set index

This document defines how the integration owner constructs one bounded package assignment without making
an implementation agent reread the architecture, unrelated packages or dependency internals.

## Machine authorities

```text
swarm/crates.toml                  package path, direct dependencies, earliest wave and assignment
swarm/function-packets.toml        primary contract/FUNCTIONS.md and package-only write scope
swarm/stages.toml                  W0–W10 package composition, shared_read_set and gate/receipt order
swarm/stage-readsets.toml          supplements and prior-handoff replacement for package reentries
swarm/launch-state.toml            current authorization only
swarm/orchestration.toml           record progression and exact control-record layouts
swarm/control-plane-schema.toml    record schema and closed reason-type registry
swarm/schemas/*.toml               field-level immutable record schemas
```

The architecture remains normative. It is not ordinary agent context. A concrete contradiction or
missing load-bearing field triggers the contract-change process.

## Deterministic context resolver

For one `(stage, package)` assignment, the materializer reads exactly:

1. root `AGENTS.md`;
2. nearest package/family `AGENTS.md`;
3. `docs/handoff/AUTHORITY_MAP.md`;
4. `swarm/ASSIGNMENT_PROTOCOL.md`;
5. only the package entry from `swarm/crates.toml`;
6. only the package/foundation entry from `swarm/function-packets.toml`;
7. the package assignment and primary contract/function packet;
8. only the selected `[[stage]]` entry from `swarm/stages.toml`;
9. the stage's `shared_read_set`;
10. for a reentry, only the matching `[[override]]` `supplements` and `additional_files`;
11. exact immutable accepted direct-dependency and `required_prior_handoffs` records;
12. exact named fixtures owned by the relevant qualification registry.

`implementation_packet` must be present inside `shared_read_set`. `machine_packet` is an integration-
owner registry and is validated but not mounted into ordinary package context.

Nothing else is mounted by default. In particular:

```text
docs/architecture/**
another package's src/**
forbidden_prior_stage_packets
all prior stage documentation
all qualification corpora
all accepted handoffs
mutable dependency branches
unselected provider artifacts
```

Every source is read by exact Git blob from one immutable base commit. The context manifest records both
exact committed source identity and normalized UTF-8/LF materialization identity; the two are never
conflated.

## Base assignment versus reentry

A **base** assignment is the package's earliest wave from `swarm/crates.toml`. The primary function
packet defines the full package behavior for that base implementation.

A **reentry** extends an already accepted package in a later stage. It receives:

- the same assignment and primary function contract;
- only current-stage `shared_read_set`;
- only exact override `supplements` and `additional_files`;
- immutable prior package API/configuration/evidence handoffs from `required_prior_handoffs`;
- no `forbidden_prior_stage_packets`;
- no prior package source beyond its own package worktree;
- no dependency implementation internals.

Example:

```text
W3:search-publication
  FUNCTIONS.md
  W3 implementation/qualification shared_read_set

W7:search-publication
  same FUNCTIONS.md
  W7 lifecycle shared_read_set
  search-publication/W7_HARDENING.md
  accepted W3 search-publication public handoff

W10:search-publication
  same FUNCTIONS.md
  W10 optional-depth shared_read_set
  search-publication/P18_SCALE.md
  scale-profile template
  accepted W7 search-publication handoff
  accepted G5/P15 and candidate-specific inputs
```

This prevents a P18 migration agent from reinterpreting the entire W3 or W7 implementation history.

## Issuance record chain

The integration owner never turns a draft directly into implementation authority. The exact chain is:

```text
swarm/context-drafts/<stage>/<package>.toml
  → swarm/context-manifests/<package>/<context_record_sha256>.toml

swarm/ticket-drafts/<stage>/<package>.toml + context manifest
  → swarm/tickets/<package>/<ticket_id>.toml

assignment ticket + context manifest
  → swarm/leases/<package>/<lease_id>.toml

writer acknowledgement
  → swarm/leases/<package>/events/<event_id>.toml

package implementation
  → swarm/submissions/<package>/<submission_id>.toml
  → swarm/reviews/<package>/<review_id>.toml
  → swarm/handoffs/<package>/<handoff_id>.toml
```

The assignment ticket follows `swarm/schemas/assignment-ticket-v1.toml` and binds:

```text
ticket identity and stable operation ID
package, stage and issue time
distinct writer/reviewer plus integration issuer
immutable repository/base commit and opaque worktree
exact package write scope and feature profile
materialized context record/artifact identities
assignment and instruction digests
accepted dependency handoffs
fixtures
bounded command/evidence requirements
explicit unavailable checks
line and context limits
signed payload identity and integration-owner signature
```

A lease is a separate immutable record. It has no automatic expiry. Implementation begins only after an
append-only `ACKNOWLEDGED` event with reason `WRITER_ACKNOWLEDGED`.

The ticket cannot replace or widen the machine read set. A new required file is a reviewed registry
change, not an ad-hoc mount.

## Authorization preflight

Before materializing context or issuing records, the integration owner verifies:

1. package exists in package and function registries;
2. package appears in the selected stage's `packages` list;
3. selected stage wave is not earlier than the package registry wave;
4. package's first stage equals its earliest registered wave;
5. every later stage has exactly one matching override and every earliest stage has none;
6. override `base_stage` and immediate `prior_stage` are correct;
7. override declares replacement/prior-handoff-only/no-dependency-source/no-shared-edit floors;
8. every context source and selector exists and resolves exactly once at the immutable base commit;
9. every forbidden prior-stage or architecture/dependency source is absent;
10. static context is at most sixteen files and emits one bounded writer artifact;
11. `swarm/launch-state.toml` authorizes the package now;
12. every direct dependency and prior stage has an accepted immutable package/API handoff;
13. all required provider/artifact/profile identities are exact and independently qualified;
14. writer and reviewer identities are valid and different;
15. no competing active package lease exists;
16. all control records validate against the exact closed schemas/reason registries;
17. every GitHub Actions workflow remains `workflow_dispatch`-only, read-only and credential-free.

A package's presence in stage/read-set registries or draft directories is not authorization.

## Write enforcement

The writer may modify only the exact package `write_scope` from `swarm/function-packets.toml`. The
matching override must repeat, not widen, that scope.

The integration owner rejects:

- edits to another package;
- root Cargo/lockfile/toolchain/CI changes;
- edits to architecture, registries, assignments, shared fixtures or central qualification evidence;
- dependency/provider/artifact/profile selection absent from the ticket;
- self-issued package/wave/Product Pulse/G6 records;
- new public types, ports or reason codes bypassing the contract-change process;
- line growth beyond the package hard stop.

Shared changes are separate integration-owner commits.

## Progressive examples

### W8 standalone CLI

```text
base accepted W1 eliot-search handoff
+ W8 shared_read_set
+ bins/eliot-search/W8_CLIENT.md
+ accepted W8 protocol handoff
```

The W1 packet is listed in `forbidden_prior_stage_packets` and is not mounted.

### W10 candidate evaluation

```text
accepted W9 search-eval API and accepted P15/G5 report/reviewer receipt
+ W10 shared_read_set
+ crates/search-eval/W10_OPTIONAL_EVALUATION.md
+ optional-depth baseline/probes/gate-map/fixture-owner registries
+ one candidate-specific G6 input set
```

The W4 and W9 implementation packets are forbidden. The evaluator cannot rewrite the accepted P15
baseline or self-accept G6.

## Completion and review

A package submission and handoff together bind:

```text
exact base/final commits and complete package-only diff
public API/configuration/profile manifest and digest
exact dependency/prior-stage handoff identities
unit/property/negative/fault/security results
cancellation/deadline/unknown-outcome evidence
content/disclosure audit where relevant
unavailable runtime/qualification checks
hand-written line count and split status
independent reviewer verdict and findings
compatibility class and closed consumer actions
```

The reviewer evaluates the primary function contract plus only the selected current-stage supplement.
A reentry review proves compatibility with the accepted prior public handoff; it does not reopen
unrelated prior implementation decisions.

Compilation and structural validators are merge prerequisites only. Package, wave, G5 or G6 acceptance
requires the declared executed evidence and independent review.

## Failure classes

Stop the assignment with one explicit integration issue when:

```text
CONTRACT_CHALLENGE
DEPENDENCY_HANDOFF_MISSING_OR_MISMATCHED
PRIOR_STAGE_RECEIPT_MISSING_OR_MISMATCHED
STAGE_READSET_INCOMPLETE
FORBIDDEN_PRIOR_PACKET_PRESENT
WRITE_SCOPE_VIOLATION
SHARED_CHANGE_REQUIRED
ARTIFACT_OR_PROFILE_UNSELECTED
QUALIFICATION_EVIDENCE_UNAVAILABLE
LINE_BUDGET_SPLIT_REQUIRED
SECURITY_OR_AUTHORITY_CONFLICT
CONTROL_RECORD_SCHEMA_MISMATCH
CONTROL_RECORD_DIGEST_MISMATCH
CONTROL_OPERATION_CONFLICT
```

Do not widen context, inspect another package's internals, select a provider or weaken an invariant as a
local workaround.
