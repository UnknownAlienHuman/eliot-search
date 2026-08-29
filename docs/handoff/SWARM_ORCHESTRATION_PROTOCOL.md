# Swarm orchestration protocol

This protocol turns the package registry, launch state and assignments into a reproducible execution
sequence. It governs implementation work; it does not implement Search behavior.

## Authorities

| Concern | Authority |
|---|---|
| package path, earliest wave, dependencies, assignment | `swarm/crates.toml` |
| current stage and authorized packages | `swarm/launch-state.toml` |
| assignment/handoff state transitions | `swarm/orchestration.toml` |
| required gate evidence | `swarm/gates.toml` |
| package semantics | package `AGENTS.md` + bounded assignment |
| shared contracts and ports | accepted dependency handoffs + contract/port catalogs |
| root changes, leases, acceptance and wave advancement | integration owner |

No chat transcript, branch name, Cargo feature or issue label overrides these authorities.

## State model

```text
BLOCKED
  → READY
  → LEASED
  → IMPLEMENTING
  → REVIEW
  → ACCEPTED

REVIEW → REJECTED
BLOCKED/READY/LEASED/IMPLEMENTING/REVIEW/REJECTED → SUPERSEDED
REJECTED → READY only through a new assignment revision/ticket
```

`ACCEPTED` is immutable. A later change starts a new assignment revision and produces a new accepted
handoff that explicitly supersedes the previous digest.

## Launch algorithm

### 1. Select a package

The integration owner verifies:

- package exists in `swarm/crates.toml`;
- package is authorized by `swarm/launch-state.toml`;
- every direct dependency has an accepted, non-superseded handoff;
- optional-provider prerequisites are satisfied when applicable;
- no active lease exists for the package;
- the repository base is the current accepted integration commit.

### 2. Freeze the input packet

Create an assignment ticket from `swarm/ASSIGNMENT_TICKET_TEMPLATE.md`. The ticket binds:

- package, wave/stage and write scope;
- repository/base commit;
- assignment file SHA-256;
- root/family/package instruction digests;
- accepted dependency commit and API/schema digests;
- allowed feature profile;
- required package tests/evidence;
- unique lease ID and writer identity.

The writer may not edit the ticket, assignment, root metadata or dependency packages.

### 3. Create isolated work

Use one branch/worktree from the bound base commit. Reject a writer whose branch contains unrelated
changes or whose base no longer matches the ticket. Rebase-by-guessing is forbidden: the integration
owner either issues a new ticket or accepts the old base deliberately after conflict review.

### 4. Implement inside the package

The writer reads only the bounded packet defined by root `AGENTS.md` and
`swarm/ASSIGNMENT_PROTOCOL.md`. Missing load-bearing semantics create a contract-change request; they do
not authorize a local duplicate type, store, policy engine, vendor client or fallback.

The writer records local commits and exact commands but does not change launch state or mark the work
accepted.

### 5. Submit package handoff

The handoff must include:

- ticket and lease ID;
- base/final commits;
- complete changed-file list;
- public API and port digest;
- state owned and dependency state referenced;
- invariant and typed-failure mapping;
- exact commands, raw result summaries and unavailable checks;
- dependency/license/ADR evidence;
- hand-written line count and split decision;
- residual risks and contract requests.

Use `swarm/PACKAGE_HANDOFF_TEMPLATE.md` and `swarm/API_HANDOFF_TEMPLATE.md`.

### 6. Review

The package reviewer reproduces package-local tests and validates ownership. The integration reviewer
then verifies:

- ticket/base/assignment/lease identity;
- no write outside the package scope;
- dependency graph and accepted input digests;
- absence of vendor/native types on public surfaces;
- no duplicate mutable-state owner;
- API digest and conformance fixture;
- truthful failure, coverage and unavailable evidence;
- line budget and split trigger;
- workspace compatibility at the integration commit.

A reviewer does not repair the package silently. Rejection returns exact reasons and closes or
supersedes the lease.

### 7. Accept and merge

Acceptance records an immutable handoff under the receipt layout described in
`swarm/handoffs/README.md`. Downstream packages consume the accepted commit/API digest, not the source
branch. Merge occurs in dependency order.

### 8. Advance a wave

The integration owner compares accepted package handoffs and evidence references with
`swarm/gates.toml`. A wave receipt may be issued only when all mandatory evidence is `PASS`; allowed
`UNAVAILABLE` results must be explicitly permitted by that gate and cannot satisfy a runtime/security
claim.

The accepted wave receipt and new launch-state commit are one reviewed integration change. Manual
launch-state edits without a receipt are invalid.

## Cancellation, rejection and supersession

- **Writer cancellation:** revoke the lease, preserve the branch/commits as non-authoritative evidence,
  and return the package to `READY` only after a new ticket.
- **Rejected review:** record reasons; no downstream consumer may reference the rejected API digest.
- **Stale dependency:** supersede the ticket and issue a new one with updated accepted dependency
  digests.
- **Architecture conflict:** stop the package and classify through the contract-change template; do not
  broaden the writer's read/write scope automatically.
- **Security discovery:** fail closed, revoke affected active tickets in orchestration metadata, and
  require explicit re-review.

## GitHub connector access

The integration owner follows `swarm/GITHUB_CONNECTOR_ACCESS.md`. A filtered tool-discovery result is
never evidence that GitHub is read-only.
