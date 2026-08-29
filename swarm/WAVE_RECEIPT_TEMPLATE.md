# Wave receipt

Only the integration owner issues this receipt.

## Identity

- Wave / stage:
- Gate:
- Repository base commit:
- Integration commit:
- Previous accepted wave receipt:
- Issued at:
- Verdict: `ACCEPTED | REJECTED`

## Accepted package handoffs

| Package | Final commit | API/schema digest | Handoff receipt | Reviewer |
|---|---|---|---|---|

## Dependency graph receipt

- `swarm/crates.toml` digest:
- Cargo workspace graph digest:
- Acyclic/direction result:
- Package/assignment/path parity result:
- Active feature profile:

## Gate evidence

| Evidence ID | Producer | Commit/artifact identity | Result | Raw output ref | Reviewer |
|---|---|---|---|---|---|

Every mandatory evidence ID from `swarm/gates.toml` must appear. `UNAVAILABLE` never silently becomes
`PASS`.

## Outstanding state

- Rejected/superseded tickets:
- Known failures:
- Unavailable checks:
- Contract or ADR blockers:
- Optional profiles still blocked:

## Launch-state transition

- Previous state digest:
- New state digest:
- Newly authorized packages:
- Newly conditional packages:
- Packages remaining blocked:
- Confirm no unresolved active lease:

## Acceptance checks

- All mandatory package handoffs accepted:
- All mandatory evidence `PASS`:
- Workspace policy evidence complete:
- Launch-state change in the same reviewed integration change:
- Product/runtime claims limited to executed evidence:

## Integration reviewer

- Issuer:
- Reviewer:
- Canonical receipt digest:
- Signature/ref:
