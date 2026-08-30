# Ticket issuance operations and recovery contract

**Status:** normative control-plane contract; implementation absent. The presence of this document,
schemas, validators, branches, pull requests or workflow runs does not materialize context, issue a
ticket, create a lease or authorize implementation.

This contract is interpreted with:

- `swarm/control-plane-schema.toml`;
- `swarm/schemas/types-v1.toml`;
- the eight record schemas under `swarm/schemas/`;
- `swarm/orchestration.toml`;
- `swarm/launch-state.toml`;
- `swarm/RECEIPT_CANONICALIZATION.md`.

## 1. Authority chain

The only valid P00 progression is:

```text
non-claimable context draft
→ immutable context manifest + one writer-visible artifact

non-claimable ticket draft + context manifest + launch authority
→ immutable assignment ticket

assignment ticket + context manifest + no competing lease
→ immutable writer lease

writer lease + exact writer acknowledgement
→ append-only ACKNOWLEDGED lease event

acknowledged active lease + package-only final commit
→ immutable package submission

package submission + different reviewer
→ immutable independent review

accepted independent review
→ immutable package handoff

accepted W0 package handoffs + G0 evidence
→ separate W0 receipt and reviewed launch-state advance
```

No record is inferred from a prior record. Each arrow is an explicit operation with preconditions,
idempotency identity, cancellation/deadline handling, exact post-write readback and a durable operation
receipt.

## 2. Canonical primitives

### 2.1 Repository paths

`RepositoryRelativeSafePath` uses the lexical grammar:

```text
^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$
```

and the additional semantic rules:

```text
forward_slashes_only
no_empty_segment
no_dot_or_dotdot_segment
no_leading_slash
case_sensitive
```

The grammar alone is insufficient because `.` and `..` match its character class. Absolute paths,
backslashes, repeated separators, dot segments, NUL and traversal are rejected before any filesystem or
Git operation.

`RepositoryGlob` is limited to a package-directory recursive scope such as
`crates/search-contracts/**`. Negation, alternation, shell expansion and dot segments are forbidden.

### 2.2 Git identities

`GitObjectId` is a full, algorithm-tagged immutable identifier:

```text
sha1:<40 lowercase hex>
sha256:<64 lowercase hex>
```

Abbreviated object IDs, branch names, tags, `HEAD`, `latest` and moving refs are not immutable inputs.

### 2.3 Actors and worktrees

`ActorIdentity` is one of:

```text
actor:user:<id>
actor:service:<id>
actor:reviewer:<id>
actor:integration:<id>
```

Writer and reviewer identities are distinct. Display names are not identities.

`OpaqueWorktreeRef` is an orchestration locator beginning with `worktree:`. It is not an operating-system
path and never replaces the separate immutable base commit.

### 2.4 Optional values

TOML has no `null`. Every optional field uses `OptionalV1`:

```text
state = ABSENT | PRESENT
value = canonical wrapped value or empty string
```

`ABSENT` requires an empty value. `PRESENT` requires a non-empty value valid for the wrapped type. Field
omission, `null` and ad-hoc sentinels are rejected.

## 3. Record identity and signatures

Two SHA-256 values have different meanings:

```text
signed_payload_sha256
  exact UTF-8/LF bytes from byte zero through the LF immediately before [signature]

exact_record_file_sha256
  every exact committed file byte, including [signature]
```

The v1 field `signature.record_sha256` stores `signed_payload_sha256`. The complete-file digest is stored
only by an external consumer, operation receipt or Git path/blob manifest. A fixed-point self-hash is
forbidden.

Every immutable reference binds:

```text
repository
immutable commit
repository-relative path
Git blob ID
exact_record_file_sha256
closed record kind
```

Readback recomputes the Git blob identity, exact complete-file digest, embedded signed-payload digest and
signature binding.

## 4. Exact record layouts

```text
swarm/context-manifests/<package>/<context_record_sha256>.toml
swarm/tickets/<package>/<ticket_id>.toml
swarm/leases/<package>/<lease_id>.toml
swarm/leases/<package>/events/<event_id>.toml
swarm/submissions/<package>/<submission_id>.toml
swarm/reviews/<package>/<review_id>.toml
swarm/handoffs/<package>/<handoff_id>.toml
swarm/supersessions/<record_kind>/<receipt_id>.toml
```

`context_record_sha256` is the external complete-file digest of the context manifest. `handoff_id` is
append-only record/path identity; `api_schema_digest` is public-surface identity and may remain unchanged
across different accepted implementation or evidence revisions.

## 5. Stable operation identity

Every operation input has:

```text
operation kind
schema version
repository
immutable base commit
actor
canonical ordered input manifest
deadline or explicit ABSENT
cancellation token or explicit ABSENT
```

The operation ID is:

```text
SHA-256(domain separator || canonical input manifest bytes)
```

Required domain separators:

```text
materialize_context_v1
issue_assignment_ticket_v1
issue_writer_lease_v1
acknowledge_writer_lease_v1
record_package_submission_v1
record_independent_review_v1
publish_package_handoff_v1
supersede_control_record_v1
recover_control_operation_v1
```

The same operation ID with byte-identical canonical input returns the original durable result. The same
operation ID with different canonical input fails with `CONTROL_OPERATION_CONFLICT`.

## 6. Context materialization

`materialize_context` consumes one context draft and one immutable base commit.

Preconditions:

1. draft is `UNMATERIALIZED_DRAFT` and non-claimable;
2. package/stage agree with launch and stage registries;
3. every declared source exists at the exact base commit;
4. every selector belongs to the closed selector grammar and resolves exactly once;
5. required accepted handoffs resolve by immutable record and public API digest;
6. no forbidden architecture or dependency implementation source is present;
7. source/fragment/artifact budgets are satisfied;
8. target context-manifest path does not exist.

For every source, the result records:

```text
declared order
repository path
Git blob ID
exact source SHA-256 and byte length
materialization mode UTF8_LF
normalized payload SHA-256 and byte length
```

The operation emits one immutable writer-visible artifact and one `context_manifest_v1`. It commits,
reads back and verifies both before success.

## 7. Assignment ticket issuance

`issue_assignment_ticket` consumes a valid ticket draft, materialized context, launch authority and
accepted prerequisites.

Preconditions:

1. draft is `DRAFT_ONLY_NOT_ISSUED`, non-claimable and not a prior ticket instance;
2. package is currently authorized or every conditional prerequisite is accepted;
3. writer and reviewer are assigned, valid and distinct;
4. base commit, package, stage, context and write scope agree across all inputs;
5. instruction, fixture, dependency, command, evidence and unavailable-check sets are complete;
6. target ticket path does not exist.

The result is one immutable `assignment_ticket_v1`. Ticket presence does not create a lease and does not
permit implementation.

## 8. Lease issuance and acknowledgement

`issue_writer_lease` consumes one assignment ticket and its exact materialized context.

Preconditions:

1. ticket and context pass schema/signature/exact-file readback;
2. ticket is current and not superseded;
3. no active non-superseded lease overlaps the package write scope;
4. writer/reviewer/base/stage/scope/profile/dependencies equal the ticket;
5. target lease path does not exist.

The result is one immutable `writer_lease_v1`. Automatic expiry is forbidden.

`acknowledge_writer_lease` is performed by the exact assigned writer. It checks ticket, lease, context,
base commit, worktree, write scope, dependency handoffs, commands/evidence and line limits. Success emits
an append-only `lease_event_v1`:

```text
event.kind = ACKNOWLEDGED
event.reason_code = WRITER_ACKNOWLEDGED
```

Implementation starts only after this event exists and passes exact readback.

Lease lifecycle mappings are closed:

```text
ACKNOWLEDGED → WRITER_ACKNOWLEDGED
SUBMITTED    → PACKAGE_SUBMITTED
REVOKED      → LEASE_REVOKED
SUPERSEDED   → LEASE_SUPERSEDED
```

## 9. Submission, review and package handoff

`record_package_submission` requires an acknowledged active lease and a final commit descending from the
ticket base commit. It recomputes the complete changed-file set and rejects any path outside the exact
write scope. The submission includes public-surface manifests/digests, raw command outcomes, evidence,
unavailable checks, line counts and residual state. Optional configuration absence is `ABSENT`, never
`null`.

`record_independent_review` requires a reviewer different from the writer and binds the exact submission
record, digest and final commit. Acceptance requires independent recomputation of scope, contract,
dependency, evidence, public-surface and line-budget claims.

`publish_package_handoff` requires an accepted independent review with no unresolved blocking finding.
The integration owner recomputes all public/evidence identities and emits one immutable
`package_handoff_v1`. The handoff cannot accept a gate or wave.

Corrections never mutate an accepted record. `supersede_control_record` emits a new replacement record and
one immutable `supersession_receipt_v1`; both old and replacement records remain byte-identical.

Supersession reasons are closed:

```text
RECORD_CORRECTION
RECORD_REPLACEMENT
AUTHORITY_REVOKED
CONTRACT_SUPERSEDED
EVIDENCE_SUPERSEDED
```

Consumer actions are closed:

```text
NO_ACTION
REVALIDATE_COMPATIBILITY
ADOPT_ADDITIVE_SURFACE
MIGRATE_BEFORE_STAGE
BLOCK_UNTIL_MIGRATED
```

## 10. Cancellation, deadlines and unknown outcomes

Cancellation or deadline observed before mutation returns a typed cancellation/deadline failure and
creates no target record.

After a mutation may have occurred, transport failure returns an unknown outcome. Blind retry is
forbidden. `recover_control_operation` performs read-only inspection of:

```text
operation ledger
intended target path
Git object identity
signed payload digest
exact complete-file digest
canonical input identity
```

Recovery returns exactly one disposition:

```text
RECOVERED_SUCCEEDED
  target exists and matches the original canonical input

SAFE_TO_RETRY
  target is absent and operation/Git evidence proves mutation absence

CONFLICT
  target exists with another input identity or immutable record digest

PRESERVE_OUTCOME_UNKNOWN
  evidence cannot distinguish absence from an unobserved write
```

Recovery does not rewrite or silently repair a record.

## 11. Closed failure registry

`ClosedReasonCode` contains exactly:

- `DRAFT_NOT_VALID`
- `DRAFT_NOT_NONCLAIMABLE`
- `CONTEXT_DRAFT_INVALID`
- `CONTEXT_SOURCE_MISSING`
- `CONTEXT_SOURCE_NOT_UTF8`
- `CONTEXT_SELECTOR_UNSUPPORTED`
- `CONTEXT_SELECTOR_NOT_UNIQUE`
- `CONTEXT_FORBIDDEN_PATH`
- `CONTEXT_HANDOFF_MISSING`
- `CONTEXT_HANDOFF_MISMATCH`
- `CONTEXT_BUDGET_EXCEEDED`
- `CONTEXT_MATERIALIZATION_CANCELLED`
- `CONTEXT_MATERIALIZATION_OUTCOME_UNKNOWN`
- `TICKET_PREREQUISITE_MISSING`
- `TICKET_WRITER_REVIEWER_CONFLICT`
- `TICKET_ISSUE_OUTCOME_UNKNOWN`
- `TICKET_OPERATION_CONFLICT`
- `PACKAGE_LEASE_CONFLICT`
- `LEASE_ACKNOWLEDGEMENT_MISMATCH`
- `LEASE_REVOKED_OR_SUPERSEDED`
- `SUBMISSION_SCOPE_VIOLATION`
- `SUBMISSION_DIFF_INCOMPLETE`
- `SUBMISSION_EVIDENCE_INCOMPLETE`
- `SUBMISSION_LINE_BUDGET_VIOLATION`
- `REVIEW_NOT_INDEPENDENT`
- `REVIEW_RECALCULATION_MISMATCH`
- `PACKAGE_HANDOFF_NOT_ACCEPTED`
- `CONTROL_RECORD_SCHEMA_MISMATCH`
- `CONTROL_RECORD_DIGEST_MISMATCH`
- `CONTROL_OPERATION_CONFLICT`
- `CONTROL_RECORD_QUARANTINED`

Unknown failure values are schema errors. Lease lifecycle, supersession and consumer-action registries are
separate because successful lifecycle events and compatibility instructions are not operation failures.

## 12. Manual validation boundary

All repository workflows are `workflow_dispatch`-only, use `contents: read` and disable checkout
credential persistence. Structural validation may prove schema closure, exact layout parity, zero state
and workflow policy. It does not issue records or count as package, gate or wave acceptance.

Current authority remains:

```text
P00 / W0
authorized package: search-contracts
conditional packages: search-domain, search-ports
materialized contexts: 0
issued tickets: 0
active leases: 0
accepted package handoffs: 0
```
