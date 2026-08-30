# P00 non-claimable draft control plane

**Status:** preparation only. No assignment ticket, materialized context, writer lease, implementation
submission, accepted handoff or W0 receipt exists.

This packet turns the P00 bootstrap sequence into three precise pre-issuance drafts without pretending
that a writer, reviewer, base commit or dependency handoff has been selected.

## 1. Records and authority

```text
context draft
  → issuance-time immutable context manifest

ticket draft + context manifest
  → integration-owner issued assignment ticket

issued ticket
  → integration-owner issued writer lease

writer lease
  → writer ACKNOWLEDGED lease event

acknowledged active lease
  → package-only implementation and submission

submission
  → independent package/integration review

accepted review
  → append-only package/API handoff

three accepted W0 package handoffs + G0 evidence
  → W0 wave receipt and separate launch-state advance
```

Drafts live under `swarm/ticket-drafts/` and `swarm/context-drafts/`. They are not part of the
`BLOCKED → READY → LEASED` orchestration state machine and cannot authorize work.

Issued records use these exact layouts:

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

Only the integration owner creates or supersedes tickets, contexts, leases, submissions and accepted
handoffs. The writer modifies only the leased package worktree and supplies submission evidence; the
writer cannot edit control-plane records directly or self-accept.

## 2. Context materialization

Each P00 draft names an exact source-file list plus exact registry selectors. At issuance, the integration
owner:

1. checks the selected immutable base commit and current launch state;
2. reads every source file by exact Git blob from that commit;
3. extracts only the declared registry entries;
4. records Git blob, exact-byte SHA-256/length and normalized UTF-8/LF SHA-256/length for every source;
5. concatenates sources and fragments in the declared order with bounded headers;
6. publishes one immutable writer-visible context artifact and one context manifest;
7. independently reads back the committed manifest and artifact;
8. binds their immutable identities into the assignment ticket.

The writer receives one context artifact, not the full repository and not the architecture master. This
keeps the mounted artifact count bounded even where `search-contracts` requires the complete P00 pack.

Changing any source byte, registry selector, accepted handoff, ordering or base commit creates a different
context and ticket. A context artifact cannot be amended after writer acknowledgement.

## 3. P00 issuance sequence

### Draft A — `search-contracts`

Launch state currently classifies this package as authorized. The draft is still non-claimable because
writer/reviewer identities, base commit, exact file/snippet digests, context identity, ticket identity and
lease identity remain unresolved.

After those fields are frozen, the integration owner may materialize the context, issue exactly one
assignment ticket, issue exactly one non-conflicting lease and record the writer acknowledgement.
Acceptance requires the package/API handoff and all package-specific P00 evidence.

### Draft B — `search-domain`

Cannot be issued until the accepted `search-contracts` package handoff and public API/schema digest are
bound. It consumes the accepted public contract only, never the contracts implementation source.

### Draft C — `search-ports`

Has the same contracts prerequisite and may run in parallel with `search-domain` only after both receive
independent tickets, contexts and leases bound to the same accepted contracts handoff.

## 4. Draft non-claims

Every committed ticket draft must retain:

```text
status = DRAFT_ONLY_NOT_ISSUED
claimable = false
authorizes_implementation = false
creates_lease = false
base_commit = UNSELECTED
writer = UNASSIGNED
reviewer = UNASSIGNED
ticket_digest = UNAVAILABLE
context_digest = UNAVAILABLE
```

Every context draft remains `UNMATERIALIZED_DRAFT` with no selected base commit, manifest or artifact.

A draft may describe launch eligibility and prerequisites, but it may not use `READY`, `LEASED`,
`IMPLEMENTING`, `REVIEW` or `ACCEPTED` as its record state.

No draft appears in `swarm/tickets/`, `swarm/context-manifests/`, `swarm/leases/`,
`swarm/submissions/`, `swarm/reviews/`, `swarm/handoffs/` or `swarm/supersessions/`.

## 5. Writer acknowledgement

Before implementation, the writer acknowledges exactly:

- ticket record ref and exact complete-file digest;
- lease record ref and exact complete-file digest;
- package and stage;
- immutable base commit and worktree reference;
- context manifest/artifact identities;
- write scope and feature profile;
- accepted dependency handoff identities;
- required commands/evidence and explicitly unavailable checks;
- line budget and split threshold.

Acknowledgement is an append-only `lease_event_v1` with kind `ACKNOWLEDGED` and reason
`WRITER_ACKNOWLEDGED`. A mismatch stops work. The writer cannot request an informal context addition;
the integration owner must issue a new or superseding context, ticket and lease.

## 6. Submission and review

A submission binds final commit, complete changed-file list, public API/schema digest, dependency
handoffs, raw command outcomes, unavailable checks, line count and contract-change records. Optional
configuration absence is encoded as explicit `OptionalV1` `ABSENT`, never TOML `null`.

The independent review separately checks:

- ticket/context/lease identity;
- package-only diff;
- primary contract and stage obligations;
- dependency and ownership boundaries;
- tests, failure/cancellation/recovery behavior;
- API and canonicalization compatibility;
- security/content-disclosure rules;
- line/split budget;
- residual risks and unavailable evidence.

Package acceptance is not gate or wave acceptance. The integration owner creates the append-only package
handoff only after the review receipt is accepted. The handoff filename uses `handoff_id`; the
`api_schema_digest` is public-surface identity, not record-path identity.

## 7. Closed reason surfaces

Operation failures use the exact `ClosedReasonCode` registry. Successful/revoked lease lifecycle events
use `LeaseEventReasonCode`; supersession records use `SupersessionReasonCode`; compatibility follow-up
uses `ConsumerActionCode`. Unknown reason or action values fail closed.

## 8. Hard stops

- draft copied into an issued-record directory without issuance-time materialization;
- mutable branch, `latest`, unresolved base or unassigned writer/reviewer in an issued record;
- context source or registry selector not present at the exact base commit;
- architecture master, another package source tree or unlisted previous-stage implementation mounted;
- second active lease for one package;
- implementation begins before the `ACKNOWLEDGED` lease event;
- write scope wider than the package registry;
- dependency branch or implementation source substituted for an accepted public handoff;
- package writer edits ticket/context/lease/review/handoff/launch records;
- compilation or structural validation represented as package/G0/W0 acceptance;
- conditional domain/ports ticket issued before accepted contracts handoff;
- automatic or externally chained GitHub Actions trigger.

## 9. Current disposition

```text
P00 ticket drafts:          3
P00 context drafts:         3
materialized contexts:      0
issued tickets:             0
active writer leases:       0
lease acknowledgement:      absent
submissions:                 0
accepted package reviews:   0
accepted package handoffs:  0
W0 receipt:                 absent
launch authority:           P00 / search-contracts only
```
