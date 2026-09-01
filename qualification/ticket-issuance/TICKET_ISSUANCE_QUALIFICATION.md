# Ticket issuance control-plane qualification

**Status:** designed, not executed.  
**Owner:** integration owner with an independent evidence reviewer.  
**Scope:** P00 assignment/control-plane operations only; no Search business capability.

This packet qualifies the machine contract that converts bounded non-claimable drafts into immutable
context, ticket, lease, submission, review and handoff records. It does not authorize any conversion.

## Inputs

```text
swarm/control-plane-operations.toml
swarm/control-plane-schema.toml
swarm/schemas/types-v1.toml
swarm/schemas/*-v1.toml
swarm/orchestration.toml
swarm/launch-state.toml
swarm/ticket-drafts/**
swarm/context-drafts/**
qualification/ticket-issuance/baseline.toml
qualification/ticket-issuance/fixtures.toml
qualification/ticket-issuance/probes.toml
```

The qualification corpus contains synthetic descriptors only. It contains no real actor assignment,
source body, credential, absolute local path, issued record or accepted authority.

## Operation closure

The exact operation set is:

```text
validate_ticket_draft
validate_context_draft
materialize_context
issue_assignment_ticket
issue_writer_lease
acknowledge_writer_lease
revoke_or_supersede_lease
record_package_submission
record_independent_review
publish_package_handoff
supersede_control_record
recover_control_operation
```

The first two are pure. Nine are append-only mutations with stable operation identities. Recovery is
read-only and returns one of four closed dispositions.

## Qualification layers

### QTI-0 — structural closure

Prove machine registry and corpus closure:

- 12 exact operations with one owner and operation class each;
- 8 exact immutable record schemas;
- 31 exact typed failure values;
- 4 exact recovery dispositions;
- 52 unique synthetic fixture descriptors;
- 64 unique mandatory probes;
- all fixture/probe references resolve;
- every failure code has at least one negative probe;
- every recovery disposition has at least one probe;
- all probe results remain `UNAVAILABLE` before execution;
- all workflows remain manual-only/read-only/credential-free;
- all authority directories remain zero-state.

A QTI-0 PASS is structural only.

### QTI-1 — pure and canonical behavior

Execute deterministic conformance for:

- ticket/context draft validation;
- safe paths, full Git object IDs and explicit OptionalV1 values;
- exact P00 context closure and ceilings;
- source/selector canonical order;
- signed-payload versus complete-file digest separation;
- signature actor/payload binding;
- unknown field/type/enum/reason rejection.

Equal exact inputs must produce byte-identical outputs or typed failures without mutation.

### QTI-2 — append-only mutation and recovery

Execute every mutation under:

- success;
- clean cancellation before publish;
- timeout/transport loss after possible write;
- same operation ID/same input retry;
- same operation ID/different input conflict;
- crash between preparation and joint commit;
- exact post-write Git blob/file/signature readback;
- partial/unreadable record quarantine.

Multi-record operations commit their records in one Git commit. In particular:

```text
package_submission_v1 + lease_event_v1/SUBMITTED
replacement_record + supersession_receipt_v1
```

Partial apparent success is forbidden.

### QTI-3 — authority and noninterference

Prove:

- draft presence creates no authority;
- ticket presence creates no lease;
- lease presence without ACKNOWLEDGED creates no implementation authority;
- terminal lease events remove authority immediately;
- package writer cannot write integration-owned records;
- independent reviewer differs from writer;
- package handoff cannot accept a gate/wave or advance launch state;
- structural/workflow success cannot create any control record;
- conditional P00 packages remain blocked without accepted `search-contracts` handoff.

## Evidence model

Every probe starts:

```text
result = UNAVAILABLE
raw_output_ref = ""
reviewer_receipt_ref = ""
```

A probe may become `PASS` only when both immutable refs are populated and independently reviewed. A prose
summary, green workflow, branch, pull request or inferred outcome is not evidence. An unavailable runtime
or platform check remains `UNAVAILABLE`.

## Acceptance rule

Ticket issuance conformance is accepted only when:

```text
all 64 mandatory probes = PASS
all raw outputs exact and immutable
all reviewer receipts independent and accepted
all 31 failure codes observed where declared
all 4 recovery dispositions observed where declared
zero hidden skips or inferred passes
zero authority mutation by the qualification harness
```

Acceptance of this qualification still does not accept `search-contracts`, G0 or W0. It only qualifies
the integration control-plane implementation when that implementation exists.

## Current disposition

```text
operation registry:          designed
fixture descriptors:         52
mandatory probes:            64
executed probes:              0
PASS:                         0
FAIL:                         0
UNAVAILABLE:                 64
real control records:         0
control-plane implementation: absent
launch authority:             P00 / search-contracts only
```
