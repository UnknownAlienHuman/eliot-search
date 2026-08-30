# P00 non-claimable draft control plane

**Status:** preparation only. No assignment ticket, writer lease, implementation submission, accepted
handoff or W0 receipt exists.

This packet turns the P00 bootstrap sequence into three precise pre-issuance drafts without pretending
that a writer, base commit or dependency handoff has been selected.

## 1. Records and authority

```text
context draft
  → issuance-time immutable context manifest

ticket draft
  → integration-owner issued assignment ticket

issued ticket
  → writer acknowledgement and writer lease

writer lease
  → package-only implementation and submission

submission
  → independent package/integration review

accepted review
  → append-only package/API handoff

three accepted W0 packages + G0 evidence
  → W0 wave receipt and separate launch-state advance
```

Drafts live under `swarm/ticket-drafts/` and `swarm/context-drafts/`. They are not part of the
`BLOCKED → READY → LEASED` orchestration state machine and cannot authorize work.

Issued records use these layouts:

```text
swarm/tickets/<package>/<ticket-id>.toml
swarm/context-manifests/<package>/<context-digest>.toml
swarm/leases/<package>/<lease-id>.toml
swarm/submissions/<package>/<submission-id>.toml
swarm/reviews/<package>/<review-id>.toml
swarm/handoffs/<package>/<api-digest>.toml
```

Only the integration owner creates or supersedes tickets, contexts and leases. The package writer may
create a package-local implementation and submit the integration-owned submission record contents, but
cannot edit control-plane records directly or self-accept.

## 2. Context materialization

Each P00 draft names an exact source-file list plus exact registry selectors. At issuance, the integration
owner:

1. checks the selected base commit and current launch state;
2. reads every source file from that exact commit;
3. extracts only the declared registry entries;
4. records SHA-256 for every source/snippet;
5. concatenates them in the declared order with path/selector headers;
6. normalizes UTF-8/LF without rewriting semantic content;
7. publishes one immutable context artifact and manifest;
8. binds its digest into the assignment ticket.

The writer receives one context artifact, not the full repository and not the architecture master. This
keeps the mounted artifact count bounded even where `search-contracts` requires the complete P00 pack.

Changing any source byte, registry selector, accepted handoff or base commit creates a different context
and ticket. A context artifact cannot be amended after writer acknowledgement.

## 3. P00 issuance sequence

### Draft A — `search-contracts`

Launch state currently classifies this package as authorized. The draft is still non-claimable because
writer/reviewer identities, base commit, exact file/snippet digests, context digest, ticket digest and
lease ID remain unresolved.

After those fields are frozen, the integration owner may issue exactly one writer ticket. Acceptance
requires the package/API handoff and all package-specific P00 evidence.

### Draft B — `search-domain`

Cannot be issued until the accepted `search-contracts` package handoff and public API/schema digest are
bound. It consumes the accepted public contract only, never the contracts implementation source.

### Draft C — `search-ports`

Has the same contracts prerequisite and may run in parallel with `search-domain` only after both receive
independent tickets bound to the same accepted contracts handoff.

## 4. Draft non-claims

Every committed draft must retain:

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

A draft may describe launch eligibility and prerequisites, but it may not use `READY`, `LEASED`,
`IMPLEMENTING`, `REVIEW` or `ACCEPTED` as its record state.

No draft appears in `swarm/tickets/`, `swarm/leases/`, `swarm/submissions/`, `swarm/reviews/` or
`swarm/handoffs/`.

## 5. Writer acknowledgement

Before implementation, a writer acknowledges exactly:

- ticket ID and canonical digest;
- lease ID;
- package and stage;
- base commit and worktree;
- context manifest/artifact digest;
- write scope;
- accepted dependency handoff digests;
- required commands/evidence and explicitly unavailable checks;
- line budget and split threshold.

A mismatch stops work. The writer cannot request an informal context addition; the integration owner
must issue a superseding ticket/context.

## 6. Submission and review

A submission binds final commit, complete changed-file list, public API/schema digest, dependency
handoffs, raw command outcomes, unavailable checks, line count and contract-change records.

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
handoff only after the review receipt is accepted.

## 7. Hard stops

- draft copied into the issued-ticket directory without issuance-time digest materialization;
- mutable branch, `latest`, unresolved base or unassigned writer in an issued record;
- context source or registry selector not present at the exact base commit;
- architecture master, another package source tree or unlisted previous-stage implementation mounted;
- second active lease for one package;
- write scope wider than the package registry;
- dependency branch or implementation source substituted for an accepted public handoff;
- package writer edits ticket/context/lease/review/handoff/launch records;
- compilation or structural validation represented as package/G0/W0 acceptance;
- conditional domain/ports ticket issued before accepted contracts handoff.

## 8. Current disposition

```text
P00 ticket drafts:       3
materialized contexts:   0
issued tickets:          0
active writer leases:    0
submissions:              0
accepted package reviews:0
accepted package handoffs:0
W0 receipt:              absent
launch authority:        P00 / search-contracts only
```
