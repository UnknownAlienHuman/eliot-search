# Writer lease

This record is issued by the integration owner after an immutable assignment ticket and context manifest
exist. It grants only one package-local implementation lease; it does not accept the package or advance a
wave.

## Identity

- Lease ID:
- Ticket ID / canonical digest:
- Context manifest / artifact digest:
- Package:
- Stage / wave:
- Writer identity:
- Reviewer identity:
- Base commit:
- Worktree / branch:
- Issued at:
- State: `LEASED`

## Scope

- Exact write scope:
- Allowed feature profile:
- Accepted dependency handoff digests:
- Required prior-stage handoff digests:
- Maximum hand-written lines:
- Split-review threshold:

## Writer acknowledgement

The writer acknowledges that:

- only the exact context artifact is authorized;
- the architecture master and dependency implementation internals are not mounted;
- no path outside the write scope may change;
- a missing contract stops work through a contract-change request;
- source/provider/artifact selection requires an integration-owned ticket;
- the writer cannot self-review, self-accept or edit launch/control-plane records.

## Lifecycle

- A second active lease for the package is denied.
- Changing base, context, assignment, write scope or dependency digest supersedes this lease.
- Revocation requires an append-only integration-owner receipt.
- Automatic expiry is not inferred from wall-clock time.
- Submission or revocation ends implementation authority; historical records remain immutable.

## Signature

- Issuer:
- Lease canonical digest:
- Signature/receipt ref:
