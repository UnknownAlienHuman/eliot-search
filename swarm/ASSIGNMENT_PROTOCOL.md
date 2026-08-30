# Package assignment protocol

Every writer receives one stage-specific read set. The integration owner derives it from the exact
package/function/stage registries rather than copying all architecture or historical stage documents.

## Required static context

A writer reads only:

1. root/family/package instructions and `docs/handoff/AUTHORITY_MAP.md`;
2. its exact `swarm/crates.toml` package entry;
3. its exact `swarm/function-packets.toml` foundation/function entry;
4. its exact `swarm/stages.toml` current stage entry;
5. when reused after its earliest wave, its one exact `swarm/stage-readsets.toml` override;
6. one assignment and the primary function/contract packet;
7. the current stage `shared_read_set`;
8. only the override supplements/additional files, if any;
9. accepted direct-dependency and prior-stage public handoff receipts named by the ticket;
10. named fixture references owned by the qualification registry.

W0 foundation writers read only their exact P00 contract files. A reused package consumes the accepted
prior public handoff; the **prior-stage implementation packet** and dependency implementation internals
are not mounted again.

## Boundaries

- one active writer, one Cargo package, one isolated worktree;
- writer edits only the exact package-local `write_scope` from `swarm/function-packets.toml`;
- a stage override cannot widen write scope;
- root workspace, lockfile, toolchain, CI, architecture, contract pack, generated schemas, registries,
  assignments, launch state, central configuration/qualification/evidence and shared fixtures belong to
  the integration owner;
- package agents do not read another package's implementation internals to fill an API gap;
- package agents do not replay earlier stage packets when an accepted handoff is available;
- missing/contradictory semantics use `CONTRACT_CHANGE_TEMPLATE.md`; never invent a local record, port,
  reason code, adapter, provider, artifact or fallback.

## Authority

- Architecture Part I owns product semantics.
- Accepted API/configuration/evidence digests own frozen downstream and later-stage contracts.
- `swarm/crates.toml` owns exact package path, direct dependencies, earliest wave and assignment.
- `swarm/function-packets.toml` owns exact primary function/contract packet and write scope.
- `swarm/stages.toml` owns W0–W10 package composition, shared stage context and gate/receipt order.
- `swarm/stage-readsets.toml` owns replacement context for later-stage package reuse.
- `swarm/launch-state.toml` owns current permission only.
- Assignment prose cannot override a machine registry, accepted handoff or launch authority.

## Ownership rule

- shared serialized records: `search-contracts`;
- pure reusable meaning: `search-domain`;
- shared vendor-neutral traits: `search-ports`;
- configuration mechanics: `search-config`;
- capability mutable state/behavior: owning package;
- concrete platform/vendor adapters: private adapter boundary, constructed only by `eliot-searchd`;
- shared qualification/gate acceptance: integration owner plus independent reviewer where required.

## Operation contract

The package's primary `FUNCTIONS.md` or foundation contract remains authoritative across all stages.
A stage supplement adds narrow current-stage obligations but cannot rewrite or weaken the accepted base
API.

Every operation defines:

- validated inputs and sole state owner;
- output/receipt identity and pre/postconditions;
- idempotency and mutation identity;
- deadline/cancellation and unknown-outcome recovery;
- finite resource/content/disclosure bounds;
- typed failures and retryability;
- deterministic, negative, property, fault and qualification fixtures.

Partial/degraded/ambiguous/incomplete outcomes are data, not apparent success. A package cannot weaken an
operation because a dependency is unavailable; it reports the typed gap or requests a contract change.

## Ticket preflight

Before issuing a writer lease, the integration owner verifies:

1. package exists and metadata match in package/function registries;
2. the current stage contains the package and matches its earliest or later wave;
3. a later-stage reuse has exactly one override and an earliest-wave assignment has none;
4. launch state and required accepted wave/gate/lifecycle receipts authorize the package;
5. exact base commit and package write scope are fixed;
6. every direct dependency has an accepted immutable API/configuration/evidence digest;
7. every required prior-stage package handoff is accepted and matches the ticket;
8. the primary, shared-stage and override files exist and their combined static count is at most sixteen;
9. no forbidden prior-stage packet, architecture master or dependency implementation is mounted;
10. named fixture refs and unavailable checks are explicit and within their ceilings;
11. package line target and split-review threshold are recorded.

A mutable branch, prose claim, green workflow or package/stage presence is not an accepted dependency
input.

## Ticket identity

The immutable ticket binds:

```text
stage and package
base commit and writer lease
package/function/stage/read-set/launch registry digests
assignment/primary/shared/override file digests
accepted direct and prior-stage handoff digests
fixture references
write scope and line limits
required commands and explicit unavailable checks
```

Changing any load-bearing input creates a new ticket. The writer cannot add context after launch without
an integration-owner amendment and revalidation.

## Implementation order

1. turn owned current-stage invariants into failing tests;
2. consume accepted contract/port and prior-stage API digests;
3. define package-owned opaque state and public capability API changes;
4. implement canonical validation/identity/state transitions before adapters;
5. add deterministic, negative, property, fault and security fixtures;
6. implement the smallest behavior that closes them;
7. verify cancellation/deadline/crash/unknown-outcome paths;
8. complete the handoff with exact commands/raw outputs, public digest, line count and unresolved gaps.

Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed environment
and artifact identity. No wildcard/floating git dependency or baseline Python/Node runtime.

Split review is mandatory before 8,500 total hand-written lines; 10,000 including local tests is a hard
stop. Forwarding-only crates and crate-per-type decomposition are forbidden.
