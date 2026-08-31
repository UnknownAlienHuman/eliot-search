# P00 ticket-issuance advisory planner v2

**Status:** executable read-only preflight. It is not a context materializer, assignment ticket, writer
lease, package submission, independent review, package handoff, gate receipt or wave receipt.

The planner evaluates one P00 package against one immutable Git commit and emits one deterministic
non-authoritative JSON decision. Repository bytes are loaded from the selected commit's Git tree; the
working tree is never a source of truth.

Machine registry: [`../../swarm/ticket-issuance-planner-v2.toml`](../../swarm/ticket-issuance-planner-v2.toml).
Output schema: [`../../swarm/ticket-issuance-plan-schema-v2.toml`](../../swarm/ticket-issuance-plan-schema-v2.toml).

## 1. Authority boundary

The planner may:

- verify package/function/stage/launch parity for the three P00 foundation packages;
- verify the exact schema-v2 non-claimable ticket/context draft pair;
- enforce manifest-owned context ceilings: ordinary `16`, exact contracts pack `24`, registry fragments
  `6`, accepted-handoff slots `1`, writer-visible artifacts `1`;
- verify the exact manifest-closed `search-contracts` source pack;
- read and hash committed UTF-8 context source blobs;
- resolve each closed registry selector exactly once;
- verify committed accepted prerequisite `package_handoff_v1` records and supersession state;
- detect current-package context/ticket/lease/submission/review/handoff records;
- reject an already accepted W0 receipt;
- verify control-schema versions and manual/read-only/credential-free workflows;
- verify an explicit immutable base commit and distinct writer/reviewer identities;
- write one ordinary JSON file below `artifacts/ticket-issuance-plans/` or stdout.

It cannot:

```text
materialize context
issue a ticket or lease
acknowledge a lease
authorize package implementation
record a submission or review
publish a package handoff
accept G0 or W0
advance launch state
write any control record
```

Every emitted plan keeps:

```text
mutations = []
authorizes_context_materialization = false
authorizes_ticket_issuance = false
creates_writer_lease = false
authorizes_implementation = false
publishes_package_handoff = false
advances_launch_state = false
```

## 2. Immutable repository view

A complete selection supplies:

```text
base_commit
writer
reviewer
```

`base_commit` is a full algorithm-tagged immutable Git commit:

```text
sha1:<40 lowercase hex>
sha256:<64 lowercase hex>
```

Writer and reviewer use the closed `ActorIdentity` grammar and must differ.

This planner stops at `READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW`. It therefore does not select
`repository_fence.branch_or_worktree`; that locator is required later by the separately authoritative
assignment-ticket operation. The planner neither invents nor writes it.

With no selection, exact `HEAD` is inspected only to produce the expected advisory
`BLOCKED_MISSING_SELECTION`. If a supplied base is invalid, the failure is recorded and exact `HEAD` is
used only for a deterministic diagnostic plan.

Uncommitted and unstaged files never alter a plan.

## 3. Schema-v2 draft checks

The ticket draft must retain:

```text
schema_version = 2
record_kind = assignment_ticket_draft
status = DRAFT_ONLY_NOT_ISSUED
claimable = false
authorizes_implementation = false
creates_lease = false
may_be_writer_acknowledged = false
```

Its unresolved identity must use the current separated digest fields:

```text
ticket_signed_payload_sha256 = UNAVAILABLE
ticket_exact_record_file_sha256 = UNAVAILABLE
```

A draft `lease_id`, legacy `ticket_canonical_digest`, resolved actor/base/worktree or any unknown
load-bearing field is rejected.

The context draft must retain four distinct unresolved identities:

```text
materialized_context_manifest_ref
materialized_context_record_sha256
materialized_context_artifact_ref
materialized_context_artifact_sha256
```

Legacy collapsed context fields are rejected. Ticket/context package, stage, phase, wave, path, write
scope, feature profile, limits, dependency slots and sentinel state must agree.

## 4. Context budgets and exact pack

`swarm/context-drafts/manifest.toml` owns the ceilings. The planner does not hard-code an ordinary
32-source allowance.

```text
ORDINARY                 <= 16 source files
P00_EXACT_CONTRACT_PACK  <= 24 source files; search-contracts only
registry fragments       <= 6
accepted handoff slots   <= 1
writer-visible artifacts = 1
```

For `search-contracts`, source order must equal:

```text
fixed integration instructions
+ docs/contracts/p00/README.md
+ docs/contracts/p00/manifest.toml
+ the remaining required_files in manifest order
```

No ad-hoc addition is accepted. All source and selector arrays are unique and their declared counts must
match.

Context sources must be safe repository-relative regular Git blobs, UTF-8, and outside:

```text
docs/architecture/**
bins/**
crates/*/src/**
all issued control-record roots
```

The total declared source bytes are capped at 16 MiB and each source at 4 MiB for planner operation.

## 5. Closed selector grammar

Only these selector forms are accepted:

```text
swarm/crates.toml::package[name=<package>]
swarm/function-packets.toml::foundation[package=<package>]
swarm/stages.toml::stage[id=W0]
swarm/launch-state.toml::authorized_packages[<package>]
swarm/launch-state.toml::conditional_packages[<package>]
swarm/launch-state.toml::conditional_activation.<package>
```

The registry path, selected package and stage are part of the grammar. A supported selector resolving zero
or multiple semantic records is `CONTEXT_SELECTOR_NOT_UNIQUE`; unsupported spelling or identity is
`CONTEXT_SELECTOR_INVALID`.

## 6. Accepted handoffs

`--accepted-handoff` takes a repository-relative path to a committed accepted `package_handoff_v1`
instance. Repeat it only for declared direct prerequisite slots.

The planner verifies:

- regular Git blob identity at the selected commit;
- exact path `swarm/handoffs/<package>/<handoff_id>.toml`;
- `schema_version = 1`, closed record kind and `status = ACCEPTED`;
- package, W0 stage and opaque handoff ID;
- signed-payload SHA-256 immediately before `[signature]`;
- full accepted final commit exists in the same repository;
- public API/schema and error/reason digest shapes;
- exact equality among ticket dependencies, context slots and supplied handoff packages;
- no committed supersession receipt identifies the handoff as the old record.

A branch, PR, implementation source tree, API sketch or digest without the immutable accepted handoff
record is insufficient.

## 7. Control-record conflicts

Exact root metadata is limited to:

```text
<control-root>/README.md
<control-root>/.gitkeep
```

A nested file named `README.md` or `.gitkeep` is a control record, not a metadata exemption.

Any existing record below the current package's context-manifest, ticket, lease, submission, review or
handoff prefix blocks this first-issuance preview. Accepted prerequisite handoffs for other packages are
allowed only when explicitly supplied and verified. Any non-metadata wave receipt blocks P00 planning.

## 8. Output and digest

Output is stdout or a `.json` path below:

```text
artifacts/ticket-issuance-plans/
```

Every other output path and every symlinked output parent/target is rejected. The artifact directory keeps
generated JSON ignored. An output plan is not a committed control record or accepted evidence receipt.

Canonical bytes are UTF-8/LF JSON with lexicographically sorted object keys and compact separators.
`plan_sha256` follows
[`TICKET_ISSUANCE_PLANNER_DIGEST_V2.md`](TICKET_ISSUANCE_PLANNER_DIGEST_V2.md).

## 9. Closed decisions

```text
READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW
BLOCKED_MISSING_SELECTION
BLOCKED_PREREQUISITE
BLOCKED_CONFLICT
INVALID_REPOSITORY_STATE
```

Preview-ready means only that an integration owner may prepare the separately specified
`materialize_context_v1` operation. It is not orchestration state `READY`.

## 10. Qualification boundary

The 30-case corpus covers immutable-tree reads, schema-v2 fields, exact contracts-pack closure,
manifest-owned ordinary ceilings, source blob/UTF-8/path rules, selectors, conditional handoffs,
supersession/conflict boundaries, workflow policy, output fencing, deterministic digest and zero authority.

The manual Windows workflow also runs the current repository through:

```text
validate-swarm.ps1
validate-p00-ticket-drafts.ps1
validate-ticket-issuance-contracts.ps1
validate-p00-foundation-acceptance.ps1
validate-ticket-issuance-plan.ps1
```

A green run proves only read-only planner conformance. It does not create implementation authority.

## 11. Current expected result

With no selected base commit and no distinct writer/reviewer identities:

```text
package = search-contracts
decision = BLOCKED_MISSING_SELECTION
mutations = []
all authority flags = false
```
