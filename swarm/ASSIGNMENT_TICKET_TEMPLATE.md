# Assignment ticket

This ticket is issued and frozen by the integration owner. The package writer does not edit it.

## Identity

- Ticket ID:
- Lease ID:
- Package:
- Wave / stage:
- Writer:
- Reviewer:
- Issued at:
- State: `LEASED`

## Repository fence

- Repository:
- Base commit:
- Branch/worktree:
- Exact write scope:
- Allowed Cargo feature profile:

## Assignment fence

- Assignment path:
- Assignment SHA-256:
- Root `AGENTS.md` digest:
- Family `AGENTS.md` digest:
- Package `AGENTS.md` digest:
- Assignment protocol digest:
- Port catalog digest:

## Accepted dependency handoffs

| Dependency | Accepted commit | API/schema digest | Handoff receipt |
|---|---|---|---|

No dependency branch or mutable worktree is an accepted input.

## Allowed read set

List only the bounded files and receipts supplied to the writer. Architecture-master access is
exception-only through a contract-change request.

## Required output

- package implementation inside the write scope;
- package-local tests and fixtures;
- package handoff;
- API/port handoff and canonical digest;
- exact raw command outcomes;
- residual risks and contract-change requests.

## Required evidence

List assignment-specific test/evidence IDs. A missing environment-dependent check remains
`UNAVAILABLE`; it is not inferred from compilation.

## Lease rules

- One active lease for this package.
- The writer acknowledges the exact ticket before implementation.
- A base, assignment or dependency-digest change supersedes this ticket.
- The writer cannot self-accept, advance launch state or broaden scope.

## Integration signature

- Issuer:
- Ticket canonical digest:
- Signature/receipt ref:
