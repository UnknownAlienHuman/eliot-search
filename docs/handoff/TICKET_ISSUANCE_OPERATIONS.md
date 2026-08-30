# Ticket issuance and package-acceptance operations

**Status:** integration-control contract only. No ticket, context, lease, submission, review or package
handoff is created by this document.

This packet defines the exact operations that transform non-claimable drafts into append-only Swarm
control records. It is owned by the integration role, not by a package agent or `eliot-searchd`.

## 1. Authority and storage

```text
swarm/ticket-drafts/**          preparatory, non-claimable
swarm/context-drafts/**         preparatory, unmaterialized

swarm/context-manifests/**      immutable materialized context records
swarm/tickets/**                immutable issued assignment tickets
swarm/leases/**                 immutable lease issue/event records
swarm/submissions/**            immutable implementation submissions
swarm/reviews/**                immutable independent review receipts
swarm/handoffs/**               immutable accepted package/API handoffs
swarm/wave-receipts/**          separately accepted wave receipts
```

The first two directories are never valid operation outputs for issuance. Every real output is a new
file. Existing accepted records are never edited; correction or cancellation is represented by a new
supersession/revocation record.

## 2. Canonical identities

Machine record files follow `swarm/RECEIPT_CANONICALIZATION.md`:

- exact committed UTF-8/LF bytes;
- SHA-256 over exact record bytes;
- Git blob identity and repository path recorded by the consuming receipt;
- list/field order is significant;
- parsed-object reserialization is never the accepted identity.

Every mutating operation has:

```text
OperationId = SHA-256(
  ASCII("eliot-search/swarm/<operation>/v1\0")
  || canonical input-manifest exact bytes
)
```

The input manifest contains every load-bearing reference and is itself retained. Equal operation ID with
different input is `CONTROL_OPERATION_CONFLICT`.

Record IDs are opaque non-empty IDs distinct from digests. The operation ID provides idempotency; the
record digest provides immutable content identity.

## 3. Context-bundle canonicalization

A materialized writer context is one UTF-8/LF artifact assembled in the declared draft order. Every
source path must match `[A-Za-z0-9._/-]+`; other path spellings are rejected for v1.

For a repository file:

```text
--- repository-path: <path> ---\n
source-git-blob: <git-object-id>\n
source-sha256: <sha256-of-exact-blob-bytes>\n
source-bytes: <exact-byte-count>\n
materialization: UTF8_LF\n
materialized-sha256: <sha256-of-normalized-payload>\n
materialized-bytes: <normalized-byte-count>\n
--- payload ---\n
<UTF-8 source normalized from CRLF/CR to LF, exactly one final LF>
--- end-repository-path: <path> ---\n
```

For an extracted registry fragment:

```text
--- registry-selector: <registry-path>::<selector> ---\n
source-git-blob: <git-object-id>\n
source-sha256: <sha256-of-exact-registry-bytes>\n
selector-match-count: 1\n
fragment-sha256: <sha256-of-exact-selected-fragment-bytes>\n
fragment-bytes: <fragment-byte-count>\n
--- payload ---\n
<exact selected UTF-8/LF fragment, exactly one final LF>
--- end-registry-selector: <registry-path>::<selector> ---\n
```

The artifact begins with:

```text
ELIOT-SWARM-CONTEXT/1\n
package: <package>\n
stage: <stage>\n
base-commit: <git-object-id>\n
context-draft-sha256: <sha256>\n
source-count: <u32>\n
registry-fragment-count: <u32>\n
accepted-handoff-count: <u32>\n
--- begin-records ---\n
```

Accepted handoff metadata is appended after static records in deterministic package-name order and
contains only receipt ref, accepted commit, API/configuration/evidence digests and compatibility class.
No dependency implementation source is embedded.

## 4. Registry-selector grammar

V1 permits only closed selectors required by current drafts:

```text
package[name=<package>]
foundation[package=<package>]
stage[id=<stage>]
authorized_packages[<package>]
conditional_packages[<package>]
conditional_activation.<package>
```

The selector parser operates on the exact committed registry bytes and must return exactly one semantic
record. Zero or multiple matches is `CONTEXT_SELECTOR_NOT_UNIQUE`. A tool may not select by line number,
regex substring or parsed-table iteration order not defined by the contract.

## 5. `validate_ticket_draft`

```text
validate_ticket_draft(draft, package_registry, function_registry, stage_registry, launch_state)
  -> Result<ValidatedTicketDraft, ControlPlaneError>
```

Requires:

- `DRAFT_ONLY_NOT_ISSUED`, non-claimable and non-authorizing flags;
- unresolved writer/reviewer/base/ticket/context/lease identities;
- exact package/stage/wave/write scope/line bounds;
- launch class consistent with current launch state;
- conditional dependency slots complete and unresolved;
- non-empty deliverables, evidence and issuance requirements;
- no secret, source body, absolute local path or mutable branch ref.

Pure and retry-safe. Success does not make the package ready.

## 6. `validate_context_draft`

```text
validate_context_draft(draft, package, registries, ceilings)
  -> Result<ValidatedContextDraft, ControlPlaneError>
```

Checks exact ordered files, selectors, handoff slots, forbidden paths, unavailable checks, safe path
grammar and static/dynamic ceilings. Every static file and registry exists at the selected repository
view, but no content is read yet.

Architecture master, implementation source, another package source tree, issued control records and
undeclared prior-stage packets fail closed.

## 7. `materialize_context`

```text
materialize_context(validated_draft, exact_base_commit, accepted_handoffs,
                    repository_reader, artifact_store, operation, deadline, cancel)
  -> Result<ContextMaterializationReceipt, ControlPlaneError>
```

### Preconditions

- exact immutable base commit exists;
- package/stage launch classification is still coherent;
- every accepted dynamic handoff is append-only, independently reviewed and matches its slot;
- conditional packages have every required handoff;
- no materialized context exists for the same operation with conflicting input;
- deadline, byte/file/fragment ceilings and cancellation are finite.

### Postconditions

- every source blob and selector source is read from the exact base commit;
- exact source blob ID/SHA-256/length and materialized/fragment SHA-256/length are recorded;
- every selector matches exactly one record;
- one canonical context artifact is stored and read back by exact length/SHA-256;
- one immutable `context_manifest_v1` record binds draft/base/handoffs/sources/fragments/artifact;
- no issued ticket or lease is created.

Cancellation before artifact publication is clean. Timeout after possible artifact/manifest publication
is `CONTEXT_MATERIALIZATION_OUTCOME_UNKNOWN` until exact artifact and record readback. Retry uses the same
operation ID and never creates a differently encoded equivalent artifact.

## 8. `issue_assignment_ticket`

```text
issue_assignment_ticket(validated_draft, context_receipt, writer, reviewer,
                        branch_or_worktree, registry_digests, evidence_plan,
                        operation, deadline)
  -> Result<TicketIssueReceipt, ControlPlaneError>
```

Requires distinct non-empty writer/reviewer identities, exact base/context artifact, current launch and
dependency prerequisites, no competing issued ticket/active lease, exact instruction/registry SHA-256
and package-only write scope.

Success creates one `assignment_ticket_v1` record under the canonical package/ticket path and exact
record readback. It does not create a lease or start work.

Timeout after possible commit is `TICKET_ISSUE_OUTCOME_UNKNOWN`; recovery reads the exact operation/ticket
path and record digest. Conflicting preexisting ticket quarantines issuance.

## 9. `issue_writer_lease`

```text
issue_writer_lease(ticket, context, current_package_state, operation, deadline)
  -> Result<WriterLeaseIssueReceipt, ControlPlaneError>
```

Requires valid issued ticket, exact context, package launch eligibility and no active unsuperseded lease.
Creates one immutable `writer_lease_v1` issuance record. Automatic wall-clock expiry is forbidden.

A second active lease is `PACKAGE_LEASE_CONFLICT`. Unknown outcome is resolved by exact lease/operation
readback; never issue a new lease ID just because the receipt was lost.

## 10. `acknowledge_writer_lease`

```text
acknowledge_writer_lease(lease, writer_ack, operation)
  -> Result<LeaseEventReceipt, ControlPlaneError>
```

Writer acknowledgement must bind ticket, lease, context artifact, base commit, write scope, dependency
handoffs, evidence obligations and line bounds. A mismatch does not partially acknowledge.

Success appends one `lease_event_v1` event of kind `ACKNOWLEDGED`; the lease record itself is unchanged.
Only then may orchestration derive `IMPLEMENTING`.

## 11. `revoke_or_supersede_lease`

```text
revoke_or_supersede_lease(lease, actor, reason, replacement_refs, operation)
  -> Result<LeaseEventReceipt, ControlPlaneError>
```

Creates an append-only `REVOKED` or `SUPERSEDED` lease event. It cannot delete history or silently reuse
the old ticket/context. A replacement requires new context/ticket/lease records.

## 12. `record_submission`

```text
record_submission(acknowledged_lease, final_commit, changed_files,
                  api_candidate, raw_evidence, unavailable_checks,
                  line_report, operation, deadline)
  -> Result<SubmissionReceipt, ControlPlaneError>
```

Requires:

- final commit descends from exact base and belongs to the leased worktree/branch;
- every changed file lies in exact package write scope;
- changed-file list is complete, sorted and unique;
- public API/configuration/fixture/error digest manifests are exact;
- raw command outcomes include command/environment/artifact identities;
- unavailable checks remain explicit;
- line/split thresholds and contract-change refs are complete;
- no placeholder/fake-success/silent-fallback attestation violation.

Success appends `package_submission_v1` and a `SUBMITTED` lease event. It does not accept the package.

## 13. `record_independent_review`

```text
record_independent_review(submission, reviewer, findings, recalculation,
                          verdict, operation, deadline)
  -> Result<ReviewReceipt, ControlPlaneError>
```

Reviewer must match the ticket reviewer or an explicit independent replacement receipt and differ from
writer. Review rechecks ticket/context/lease identities, full diff/scope, dependency handoffs, primary
and stage contracts, raw outcomes, API manifests, ownership, compatibility and line budget.

Closed verdicts:

```text
ACCEPT_SUBMISSION_FOR_INTEGRATION
REQUEST_CHANGES
REJECT
SUPERSEDED
```

Success appends `independent_review_v1`. Acceptance is not a package handoff, gate or wave receipt.

## 14. `publish_package_handoff`

```text
publish_package_handoff(submission, accepted_review, integration_recheck,
                        operation, deadline)
  -> Result<PackageHandoffReceipt, ControlPlaneError>
```

Requires accepted review, exact final commit/diff/API manifests, matching dependency handoffs and no
unresolved critical finding. Integration independently recomputes record and API digests.

Success appends `package_handoff_v1` with accepted commit, API/configuration/evidence digests,
compatibility and supersession refs. It cannot advance a wave or launch state by itself.

## 15. `supersede_control_record`

```text
supersede_control_record(old_record, replacement_record, actor, reason, operation)
  -> Result<SupersessionReceipt, ControlPlaneError>
```

Both records remain immutable. The replacement must be of a compatible record kind/package/stage and
explicitly cite the old digest. Accepted historical package handoffs are never rewritten; consumers move
to the replacement only through a separately reviewed ticket/launch change.

## 16. Recovery

```text
recover_control_operation(operation_id, expected_input_digest, canonical_path, repository_reader)
  -> ControlOperationRecovery
```

Closed decisions:

```text
COMMITTED_EXACT(record_ref, digest)
NOT_COMMITTED_RETRY_SAME_OPERATION
CONFLICTING_RECORD_QUARANTINE
PARTIAL_OR_UNREADABLE_QUARANTINE
```

Recovery never infers success from a branch name, pull request, local file, comment or responding agent.
Exact committed record bytes and digest are authoritative.

## 17. Content and disclosure floors

Control records may contain repository-relative paths and opaque record/artifact refs. They may not
contain:

- source bodies, unsaved buffers or raw query text;
- credentials, tokens, pairing proofs or secret material;
- unrestricted local absolute paths or user home locations;
- dependency implementation source;
- foreign hidden membership/corpus names;
- raw Qdrant filters/point IDs or provider internals.

Raw evidence remains in separately governed artifacts and is referenced by digest.

## 18. Typed failures

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

## 19. Required structural and future runtime evidence

- every machine schema rejects unknown load-bearing fields;
- exact-byte record SHA-256 and Git blob identity fixtures;
- context UTF-8/LF normalization and exact-source/materialized dual digest fixtures;
- selector exact-one-match fixtures for every permitted grammar form;
- missing/duplicate selector, forbidden path and dependency-source negative fixtures;
- conditional ticket without accepted handoff rejected;
- writer/reviewer equality rejected;
- second active lease rejected;
- ticket/context/lease unknown-outcome exact readback;
- acknowledgement mismatch rejected;
- changed-file scope completeness and traversal/case/Unicode fixtures;
- submission/review/handoff idempotency and conflicting-retry rejection;
- reviewer independence and API digest recalculation;
- supersession leaves historical bytes immutable;
- control-record content/secret/source/path sink audit;
- zero-state repository contains no issued record;
- manual structural validator success is never package, gate or wave evidence.

## 20. Current disposition

```text
record schemas:           designed by this packet
context materializer:     not implemented
issuance operations:      not implemented
issued P00 tickets:       0
active writer leases:     0
package submissions:      0
accepted package handoffs:0
launch authority:         P00 / search-contracts eligible only after issuance
```
