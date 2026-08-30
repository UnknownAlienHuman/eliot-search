# Package assignment protocol

Every writer receives one stage-specific read set. The integration owner derives it from the exact
package/function/stage registries rather than copying all architecture or historical stage documents.

## Pre-issuance drafts

A committed file under `swarm/ticket-drafts/` or `swarm/context-drafts/` is preparation only. It must
remain `DRAFT_ONLY_NOT_ISSUED` or `UNMATERIALIZED_DRAFT`, with no writer, reviewer, base commit, context
digest, ticket digest or lease.

A draft cannot be acknowledged by a writer and cannot transition directly to `LEASED`. The integration
owner must create new immutable records in this order:

```text
materialized context manifest/artifact
→ issued assignment ticket
→ writer lease
→ writer acknowledgement
```

The materialized context binds one exact base commit, every source-file/registry-fragment SHA-256,
accepted dynamic handoffs and one writer-visible artifact digest. Changing any load-bearing input creates
a superseding context and ticket; an acknowledged context is never amended.

Conditional packages cannot receive an issued ticket until their accepted prerequisite handoffs are
bound. For P00 this means `search-domain` and `search-ports` remain unavailable until the accepted
`search-contracts` package/API handoff exists.

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

At issuance these sources are materialized into the exact immutable writer-context artifact identified by
the ticket/lease. W0 foundation writers receive only their exact P00 contract files. A reused package
consumes the accepted prior public handoff; the **prior-stage implementation packet** and dependency
implementation internals are not mounted again.

## Boundaries

- one active writer, one Cargo package, one isolated worktree;
- writer edits only the exact package-local `write_scope` from `swarm/function-packets.toml`;
- a stage override cannot widen write scope;
- root workspace, lockfile, toolchain, CI, architecture, contract pack, generated schemas, registries,
  assignments, launch state, ticket/context/lease/review records, central configuration/qualification/
  evidence and shared fixtures belong to the integration owner;
- package agents do not read another package's implementation internals to fill an API gap;
- package agents do not replay earlier stage packets when an accepted handoff is available;
- package agents cannot add files to an acknowledged context or edit their ticket/lease;
- missing/contradictory semantics use `CONTRACT_CHANGE_TEMPLATE.md`; never invent a local record, port,
  reason code, adapter, provider, artifact or fallback.

## Authority

- Architecture Part I owns product semantics.
- Accepted API/configuration/evidence digests own frozen downstream and later-stage contracts.
- `swarm/crates.toml` owns exact package path, direct dependencies, earliest wave and assignment.
- `swarm/function-packets.toml` owns exact primary function/contract packet and write scope.
- `swarm/stages.toml` owns W0–W10 package composition, shared stage context and gate/receipt order.
- `swarm/stage-readsets.toml` owns replacement context for later-stage package reuse.
- `swarm/launch-state.toml` owns current package authorization/conditional state.
- issued ticket + materialized context + active lease own the exact writer/base/read/write fence.
- non-claimable ticket/context drafts own no implementation authority.
- Assignment prose cannot override a machine registry, accepted handoff, issued ticket or launch authority.

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
8. the primary, shared-stage and override files exist and their declared static count is within bounds;
9. the context materializer records exact source/fragment digests and emits one immutable artifact;
10. no forbidden prior-stage packet, architecture master or dependency implementation is mounted;
11. named fixture refs and unavailable checks are explicit and within their ceilings;
12. writer and reviewer identities are distinct and no active package lease exists;
13. package line target and split-review threshold are recorded;
14. the issued ticket/context/lease canonical digests are independently recomputed.

A mutable branch, draft record, prose claim, green workflow or package/stage presence is not an accepted
dependency or ticket input.

## Ticket identity

The immutable issued ticket and lease bind:

```text
stage and package
base commit and writer/reviewer identities
writer lease
package/function/stage/read-set/launch registry digests
assignment/primary/shared/override file digests
materialized context manifest and artifact digest
accepted direct and prior-stage handoff digests
fixture references
write scope and line limits
required commands and explicit unavailable checks
```

Changing any load-bearing input creates a new ticket/context/lease. The writer cannot add context after
acknowledgement without an integration-owner supersession and revalidation.

## Writer acknowledgement

Before implementation the writer confirms the exact ticket ID/digest, lease, package, stage, base commit,
context artifact digest, write scope, accepted dependency/prior-stage handoffs, evidence obligations and
line budget. Any mismatch stops work; silent acceptance of a newer branch/context is forbidden.

## Implementation order

1. turn owned current-stage invariants into failing tests;
2. consume accepted contract/port and prior-stage API digests;
3. define package-owned opaque state and public capability API changes;
4. implement canonical validation/identity/state transitions before adapters;
5. add deterministic, negative, property, fault and security fixtures;
6. implement the smallest behavior that closes them;
7. verify cancellation/deadline/crash/unknown-outcome paths;
8. complete the submission with exact diff, commands/raw outputs, public digest, line count and unresolved
   gaps.

Compilation alone is not acceptance. Windows/Qdrant/redb/provider claims require executed environment
and artifact identity. No wildcard/floating git dependency or baseline Python/Node runtime.

## Submission, review and handoff

The package submission binds ticket/lease/context, base/final commit, complete changed-file list,
candidate public API/configuration digest, raw outcomes, unavailable checks and line count.

A distinct reviewer verifies scope, contract/stage behavior, dependency/ownership boundaries,
cancellation/recovery, evidence, compatibility and line budget. An accepted review permits the
integration owner to issue an append-only package/API handoff only; it is not a gate or wave receipt.

Split review is mandatory before 8,500 total hand-written lines; 10,000 including local tests is a hard
stop. Forwarding-only crates and crate-per-type decomposition are forbidden.
