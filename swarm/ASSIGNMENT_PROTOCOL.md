# Package assignment protocol

Every writer reads root/family/package instructions, `AUTHORITY_MAP.md`, its exact
`swarm/crates.toml` and `swarm/function-packets.toml` entries, one assignment, owned
configuration/qualification/stage packets, relevant ports and accepted direct dependency handoffs.
W0 foundation writers additionally read only their exact P00 primary/supplemental contract files.

## Boundaries

- one active writer, one Cargo package, one isolated worktree;
- writer edits only the exact package-local `write_scope` from `swarm/function-packets.toml`;
- root workspace, lockfile, toolchain, CI, architecture, contract pack, generated schemas, registries,
  assignments, launch state, central configuration/qualification and shared fixtures belong to the
  integration owner;
- package agents do not read another package's implementation internals to fill an API gap;
- missing/contradictory semantics use `CONTRACT_CHANGE_TEMPLATE.md`; never invent a local record, port,
  reason code, adapter, provider, artifact or fallback.

## Authority

- Architecture Part I owns product semantics.
- Accepted API/configuration/evidence digests own frozen downstream contracts.
- `swarm/crates.toml` owns exact package path, direct dependencies, wave and assignment.
- `swarm/function-packets.toml` owns exact primary function/contract packet and write scope.
- `swarm/launch-state.toml` owns current permission only.
- Assignment explains capability mission/ownership but cannot override either machine registry or launch
  authority.

## Ownership rule

- shared serialized records: `search-contracts`;
- pure reusable meaning: `search-domain`;
- shared vendor-neutral traits: `search-ports`;
- configuration mechanics: `search-config`;
- capability mutable state/behavior: owning package;
- concrete platform/vendor adapters: private adapter boundary, constructed only by `eliot-searchd`;
- shared qualification/gate acceptance: integration owner plus independent reviewer where required.

## Operation contract

The package's primary `FUNCTIONS.md` or foundation contract is authoritative for ordinary implementation
work. Every operation defines:

- validated inputs and sole state owner;
- output/receipt identity and pre/postconditions;
- idempotency and mutation identity;
- deadline/cancellation and unknown-outcome recovery;
- finite resource/content/disclosure bounds;
- typed failures and retryability;
- deterministic, negative, property, fault and qualification fixtures.

Partial/degraded/ambiguous/incomplete outcomes are data, not apparent success. A package cannot weaken an
operation contract because a dependency is unavailable; it reports the typed gap or requests a contract
change.

## Ticket preflight

Before issuing a writer lease, the integration owner verifies:

1. package exists and metadata match in both machine registries;
2. launch state and required accepted wave/gate receipts authorize the package;
3. exact base commit and package write scope are fixed;
4. every direct dependency has an accepted immutable API/configuration/evidence digest;
5. the primary function/stage/configuration/qualification packets exist and are immutable for the ticket;
6. named fixture refs and unavailable checks are explicit;
7. package line target and split-review threshold are recorded.

A mutable branch, prose claim, green workflow or package presence is not an accepted dependency input.

## Implementation order

1. turn owned invariants into failing tests;
2. consume accepted contract/port API digests;
3. define package-owned opaque state and public capability API;
4. implement canonical validation/identity/state transitions before adapters;
5. add deterministic, negative, property, fault and security fixtures;
6. implement the smallest behavior that closes them;
7. verify cancellation/deadline/crash/unknown-outcome paths;
8. complete the handoff with exact commands/raw outputs, public digest, line count and unresolved gaps.

Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed environment
and artifact identity. No wildcard/floating git dependency or baseline Python/Node runtime.

Split review is mandatory before 8,500 total hand-written lines; 10,000 including local tests is a hard
stop. Forwarding-only crates and crate-per-type decomposition are forbidden.
