# Ticket-issuance dry-run planner

**Status:** executable preflight only. The planner cannot materialize a writer context, create a ticket,
issue or acknowledge a lease, record a submission/review, publish a package handoff, satisfy a gate or
advance a wave.

The planner turns the current repository state and an optional complete issuance selection into one
canonical, read-only JSON decision. It exists to make missing prerequisites and conflicts explicit before
an integration owner performs any append-only mutation.

## 1. Authority boundary

The planner consumes, but never modifies:

```text
swarm/launch-state.toml
swarm/orchestration.toml
swarm/control-plane-schema.toml
swarm/schemas/types-v1.toml
swarm/ticket-drafts/manifest.toml
swarm/context-drafts/manifest.toml
swarm/ticket-drafts/<stage>/<package>.toml
swarm/context-drafts/<stage>/<package>.toml
swarm/crates.toml
swarm/function-packets.toml
swarm/stages.toml
swarm/stage-readsets.toml
accepted immutable prerequisite handoffs, when supplied
```

It writes only its requested ordinary output file outside all protected control-record roots. The output
is an advisory artifact, not a Swarm record and not an accepted evidence receipt.

## 2. Inputs

```text
repository root
package
optional full algorithm-tagged base commit
optional writer ActorIdentity
optional reviewer ActorIdentity
optional accepted immutable handoff descriptors
```

A real issuance candidate is complete only when all three selection fields are present:

```text
base commit
writer
reviewer
```

Supplying only part of that triple is `PARTIAL_ISSUANCE_SELECTION` and fails closed. Writer and reviewer
must be distinct. A moving ref, abbreviated object ID, display name or local absolute worktree path is not
an identity.

## 3. Repository checks

The planner verifies:

1. package identity/path/wave/write scope agree across package and function registries;
2. package belongs to the selected active stage;
3. launch classification matches `authorized_packages` or `conditional_packages`;
4. the ticket/context drafts form one exact package/stage pair;
5. both drafts remain non-claimable and unresolved;
6. every declared context source exists as a regular non-symlink repository file;
7. every declared registry selector uses the closed v1 grammar;
8. required accepted-handoff slots equal supplied immutable handoffs exactly;
9. every protected issued-record root remains zero-state except `README.md`/`.gitkeep`;
10. orchestration/control/type schema versions and paths are coherent;
11. every workflow remains manual-only, read-only and credential-free;
12. a selected base commit is a full immutable Git object and contains every declared source;
13. writer/reviewer identities match the closed actor grammar and differ;
14. no second active lease or conflicting issued record exists.

A structural planner PASS is still not permission to implement. It only means an integration owner has a
complete, conflict-free candidate input for later context materialization.

## 4. Canonical output

The planner emits UTF-8/LF canonical JSON with lexicographically sorted object keys and compact separators.
The top-level shape is fixed by `swarm/ticket-issuance-plan-schema.toml`.

Mandatory non-authority fields are always:

```text
mutations = []
authorizes_ticket_issuance = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
advances_launch_state = false
```

Closed decisions:

```text
READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW
BLOCKED_MISSING_SELECTION
BLOCKED_PREREQUISITE
BLOCKED_CONFLICT
INVALID_REPOSITORY_STATE
```

`READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW` means only that the next integration-owned operation may be
prepared. It does not mean `READY`, `LEASED` or `IMPLEMENTING` in `swarm/orchestration.toml`.

## 5. Determinism and identity

The plan contains no wall-clock time, random ID, branch-head inference or machine-local absolute path.
Its digest is:

```text
SHA-256(ASCII("eliot-search/ticket-issuance-plan/v1\0") || canonical_json_bytes)
```

Equal repository bytes and equal explicit inputs produce equal output bytes and plan digest. The digest
identifies only the advisory plan; it is not an `OperationId`, ticket ID, context ID or control-record
digest.

## 6. Failure surface

- `PACKAGE_UNKNOWN`
- `PACKAGE_STAGE_MISMATCH`
- `PACKAGE_REGISTRY_MISMATCH`
- `DRAFT_PAIR_MISSING`
- `DRAFT_PAIR_MISMATCH`
- `DRAFT_BECAME_CLAIMABLE`
- `DRAFT_IDENTITY_PREMATURELY_RESOLVED`
- `CONTEXT_SOURCE_MISSING`
- `CONTEXT_SOURCE_SYMLINK`
- `CONTEXT_SOURCE_FORBIDDEN`
- `CONTEXT_SELECTOR_INVALID`
- `HANDOFF_SLOT_UNSATISFIED`
- `HANDOFF_SET_UNEXPECTED`
- `PARTIAL_ISSUANCE_SELECTION`
- `BASE_COMMIT_INVALID`
- `BASE_COMMIT_SOURCE_MISSING`
- `ACTOR_IDENTITY_INVALID`
- `WRITER_REVIEWER_CONFLICT`
- `PROTECTED_ROOT_NOT_ZERO_STATE`
- `ACTIVE_LEASE_CONFLICT`
- `CONTROL_SCHEMA_MISMATCH`
- `WORKFLOW_POLICY_VIOLATION`
- `OUTPUT_PATH_PROTECTED`
- `OUTPUT_WRITE_FAILED`

Unknown load-bearing input or repository state fails closed.

## 7. Qualification

The conformance corpus must cover at least:

- deterministic repeated planning;
- all three P00 draft pairs;
- authorized `search-contracts` with no selection;
- conditional domain/ports without accepted contracts handoff;
- complete valid selection with distinct actors;
- partial selection;
- identical actors;
- abbreviated/moving base ref;
- missing/duplicate/claimable draft;
- source traversal/symlink/missing source;
- unexpected or missing handoff;
- issued record or active lease in zero-state repository;
- automatic workflow trigger or write permission;
- protected output path;
- proof that output contains zero mutations and every non-authority flag is false.

## 8. Current disposition

```text
planner contract:             defined
planner output authority:     none
context materializer:         absent
issued tickets:               0
active writer leases:         0
accepted package handoffs:    0
launch authority:             P00 / W0
```
