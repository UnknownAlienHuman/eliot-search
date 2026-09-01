# Implementation-to-production execution plan

## Purpose

This document is the implementation execution contract for turning the Architecture 8.4 scaffold into
a useful, secure and supportable Windows product.

It does **not** authorize implementation by itself. Current authority remains
`swarm/launch-state.toml`; every writer still requires an issued immutable ticket, materialized context,
active lease, accepted prerequisite handoffs and package-only write scope.

The plan distinguishes four different outcomes:

| Outcome | Required stage | Meaning |
|---|---:|---|
| Buildable workspace | integration bootstrap + W0 | The exact toolchain, dependency graph, public contracts and fake-port test surfaces compile reproducibly. |
| Bootable service shell | W1 | `eliot-searchd` starts, owns one data root, validates configuration, exposes the bounded local protocol, reports readiness and shuts down cleanly. No source-search claim is made. |
| Useful baseline product | W4 / accepted G2 | Admitted local sources can be indexed and queried through bounded recipes with access filtering, exact source readback, compact results, handles and continuations. |
| Release candidate | W9 / accepted G5 | The W4–W8 product has passed Windows Product Pulse, fault/recovery, protocol stress, leakage and independent acceptance review. |
| Optional depth | W10 / accepted G6 | One separately selected model, document or scale candidate adds measured value and has complete removal/rollback evidence. It is not required for the baseline release. |

## Current state and blocking facts

The repository is a complete implementation scaffold, not an implementation:

- all 45 Cargo packages exist, but their `src/` files are package boundaries only;
- there is no committed `Cargo.lock`;
- the exact Windows Rust toolchain and third-party dependency set are not selected;
- all runtime, Qdrant, parser, provider, performance and recovery evidence is absent;
- all W1+ packages are blocked;
- no materialized writer contexts, issued implementation tickets, active leases, accepted package
  handoffs, wave receipts or gate receipts exist;
- optional model, document and scale candidates remain disabled and unselected.

The first executable work is therefore P00/W0, not daemon or Qdrant implementation.

## Sources of truth

Future implementation PRs must use the following authorities instead of copying this plan into package
code:

```text
swarm/launch-state.toml                 current implementation authority
swarm/crates.toml                       package paths, dependencies, waves and line limits
swarm/function-packets.toml             exact package function/contract sources and write scopes
swarm/module-packets.toml               package-local module boundaries
swarm/coverage/package-maps/<package>/  bounded operation/document/relation map for one writer
swarm/stages.toml                       W0–W10 assignment and receipt order
swarm/stage-readsets.toml               replacement context for every later-stage re-entry
swarm/gates.toml                        required evidence for G0–G6
config/sections.toml                    exact configuration owner package/module
qualification/**                        unexecuted evidence contracts
```

If this plan conflicts with Architecture Part I, an accepted ADR, an issued immutable ticket, an
accepted handoff or launch state, the higher authority wins and the contradiction must be resolved
before implementation continues.

## Work ownership and PR discipline

1. One implementation writer owns one Cargo package, one worktree and one package PR.
2. The writer reads only the materialized package context and accepted public handoffs named by the
   ticket. The writer does not browse the architecture master or dependency source.
3. Package writers edit only their package write scope. Root Cargo files, `Cargo.lock`, toolchain,
   shared CI, central configuration registries, shared fixtures, qualification registries, release
   packaging and gate evidence belong to the integration owner.
4. A package PR begins with failing contract/property/fault tests and implements only the operations
   routed to that package in its bounded maps.
5. Normal target is the package-specific target at or below 7,500 handwritten `src/` lines. Split
   review is mandatory before 8,500; 10,000 including local tests is a hard stop.
6. No forwarding-only crate, duplicate state owner, hidden public bypass, unbounded queue, silent
   fallback, fake receipt or placeholder success is allowed.
7. A package PR may produce a submission candidate. Independent review and integration-owned handoff
   publication remain separate.
8. Parallel implementation is allowed only when all direct dependencies have accepted immutable public
   handoffs. Reused packages receive a new ticket and replacement context for the new stage.

## Integration bootstrap before package implementation

The integration owner must land a bounded bootstrap PR before or alongside the first P00 package work.

### Toolchain and dependency closure

- pin one exact stable Rust release that supports `x86_64-pc-windows-msvc`;
- pin the required Cargo tools used by the repository;
- select exact, non-floating dependency versions and generate `Cargo.lock`;
- preserve `unsafe_code = "deny"` and the existing workspace lint policy;
- run license/advisory/source checks against the pinned dependency graph;
- document every native binary or library artifact, checksum, license, origin and update policy;
- forbid automatic download or silent upgrade of Qdrant, parsers, models and document providers;
- keep optional model/document/scale dependencies behind non-default, explicitly authorized features.

### Build profiles and feature topology

The workspace must have explicit profiles for:

```text
baseline DIRECT
qualified LEXICAL/indexed
qualified CODE/current workspace
optional model candidate
optional document candidate
optional advanced-scale candidate
```

The default installed baseline must not require an optional worker or remote service. Optional feature
presence, configuration or worker readiness must not activate a capability without accepted profile and
binding authorization.

### Test and evidence harness

Provide integration-owned commands for:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --doc --workspace
cargo nextest run --workspace
cargo deny check
all structural repository validators
stage-specific qualification runners
Windows fault/restart/ACL/process tests
```

Feature-specific checks must run in separate matrices so an optional feature cannot silently alter the
accepted baseline. GitHub Actions remain manual-only and read-only; the same commands must be runnable
locally.

### Artifact and data-layout contract

Before stateful code is merged, freeze:

- application, protocol and on-disk schema version fields;
- deterministic data-root directory layout;
- ownership/ACL expectations for every directory and file;
- redb, revision CAS, Qdrant and temporary workspace locations;
- durable versus ephemeral data classification;
- migration journal and rollback marker locations;
- log/diagnostic locations and redaction policy;
- package, Qdrant and optional-worker artifact manifest formats.

No package may invent a private incompatible layout.

## W0 — Contract freeze and executable foundation

### `search-contracts`

Implement all P00 canonical IDs, bounded records, request/result schemas, reason codes, protocol records,
lifecycle records, source/revision/representation/unit/anchor identities and serialization rules.

Required properties:

- canonical encoding is deterministic and versioned;
- invalid, unknown, oversized and contradictory values fail closed;
- wire/domain types contain no vendor-native handles, raw credentials or unrestricted paths;
- every bounded collection, string, payload and depth has an enforced limit;
- recipe and result tags are exact and closed;
- opaque tokens remain opaque and cannot be treated as authorization.

Required tests include canonical golden fixtures, round trips, malformed/unknown-field rejection, bounds,
ordering and reason-code registry closure.

### `search-domain`

Implement pure state-transition and meaning kernels only:

- eligibility intersection and non-widening rules;
- currentness, assurance, partial/degraded and coverage semantics;
- deterministic ordering and tie breaking;
- lifecycle/publication/purge state transitions;
- configuration action composition;
- outcome-unknown and retry classification.

No I/O, thread runtime, persistence, vendor client or platform API belongs here.

### `search-ports`

Implement the 23 shared vendor-neutral port traits and their 80 source-exact methods, including operation
context, cancellation, deadlines, bounded streams/pages and typed failures. Provide fake ports and
conformance helpers without selecting concrete runtime adapters.

### W0 integration and G0

After independent package review:

1. publish the accepted `search-contracts` API/schema handoff;
2. issue separate `search-domain` and `search-ports` tickets against that digest;
3. verify Cargo/registry/module/coverage parity and dependency acyclicity;
4. pin the real dependency graph and commit `Cargo.lock`;
5. execute every G0 evidence item with raw output;
6. publish accepted package handoffs, the W0 receipt and G0 receipt;
7. advance launch state to W1 only after all leases/submissions are closed.

## W1 — Bootable process and control shell

W1 should produce a service that starts and stops correctly, but it must not claim source search.

### `search-config`

Implement deterministic defaults/file/environment/CLI layering, typed section dispatch, provenance,
redaction, canonical fingerprints, diffs and composite reconfiguration action plans. A single severity
enum must not erase simultaneous security, restart, rebuild, generation or gate obligations.

### `search-runtime-owner`

Implement exclusive data-root ownership, owner epoch/incarnation, stale-owner inspection, takeover
policy, heartbeats and release. Two live daemons must never own one root.

### `search-os-secrets`

Implement the Windows-bound secret store, opaque references, short-lived leases, rotation and deletion.
Secret bytes must not enter configuration, logs, panic text, crash reports, telemetry or public errors.

### `search-control-redb`

Implement versioned control schemas, migrations, transactional snapshots, compare-and-swap guards,
idempotency records, unresolved-operation recovery, quarantine and bounded counters. redb never stores a
searchable corpus or source body.

### `search-provider-protocol`

Implement bounded frame encoding/decoding, version negotiation, pairing/authentication primitives,
session state, request admission, progress, one terminal response, cancellation and disconnect cleanup.
Frame limits must be enforced before allocation.

### `eliot-searchd` W1 composition

Implement the first daemon startup graph:

```text
load and validate configuration
→ acquire data-root ownership
→ open/migrate control journal
→ resolve secret references
→ bind the authenticated local endpoint
→ publish truthful readiness
→ accept bounded shell/health requests
```

Shutdown reverses the graph, drains requests, releases leases/guards and records unresolved external
outcomes without fabricating success.

### `eliot-search` W1 shell

Implement the declared local endpoint/configuration/diagnostic command surface, stable exit codes,
machine-readable output and redacted human output. It must not bypass provider protocol or open stores
directly.

### W1 exit

Prove single-owner behavior, migration/reopen, secret non-disclosure, protocol framing limits, request
cancellation, clean startup/shutdown and hot health admission without durable query writes. Publish W1
package handoffs and receipt; G1 remains open until W2.

## W2 — Direct source spine and first end-to-end local data path

### `search-source-admission`

Implement deny-by-default source policy normalization, exact root/type/size/security checks, immutable
admission receipts and restrictive policy-change classification.

### `search-source-identity`

Implement stable physical/logical source identities, path history, rename behavior, owner generations
and collision-safe canonical keys. A path remains a locator, not identity.

### `search-source-registry`

Implement admitted roots, source bindings, memberships, source/workspace views, exact denominator
inventory and fenced namespace-owner cutover.

### `search-safe-reader`

Implement final-handle containment verification, reparse/symlink defense, stable before/after metadata
checks, bounded batch reads and no-execute Git-object/native reads. Bytes are accepted only after the
opened object is proven inside an admitted root.

### `search-revision-store`

Implement immutable revision manifests/CAS objects, residency domains, encryption-key and retention
domains, leases, exact reopen, mark roots, transition records and crash-safe recovery. Physical reuse is
allowed only across equivalent security/lifecycle domains.

### `search-materializer`

Implement bounded canonical representations for the baseline text/code inputs, encoding/newline
normalization, coordinate and loss maps, assurance classification and deterministic receipts. No parser,
renderer or provider may execute source content.

### `search-unitizer`

Implement deterministic unit boundaries, occurrence identities, native/canonical anchors, manifests,
diffs and verification.

### `eliot-searchd` W2 re-entry

Compose the accepted W1 daemon API with source admission, registry, safe read, revision, materialization
and unitization handoffs. Add only DIRECT-mode capabilities; do not read W1 implementation internals.

### W2 exit and G1

A Windows end-to-end fixture must be able to:

```text
initialize an empty data root
register an admitted source root
deny an outside or unsafe source
read an admitted file through the final-handle check
admit and reopen the exact immutable revision
materialize and unitize it deterministically
restart the daemon and recover the same authoritative state
return a bounded direct result without Qdrant
```

Execute every G1 item, including root ownership, journal recovery, deny-default admission, identity
cutover, no-execute read, residency-aware storage, anchor fixtures and exact revision readback.

## W3 — Qualified lexical index and crash-safe publication

### Package implementation

- `search-lexical`: deterministic analyzer profiles, tokenization, sparse vectors, query/document parity
  and golden fixtures.
- `search-point-identity`: canonical point keys, full digests, deterministic Qdrant UUIDs and
  collision/non-overwrite behavior.
- `search-projection-planner`: exact point-set manifests, one membership per point, deterministic diffs
  and generation requirements.
- `search-qdrant-supervisor`: exact Windows artifact qualification, process identity, containment,
  readiness, shutdown and restart policy.
- `search-qdrant-bridge`: capability/schema checks and exact bounded admin/mutation/read/query/delete
  port implementations without leaking vendor types.
- `search-publication`: serialized publication intent, stage/readback/final barrier, atomic visible-epoch
  commit, recovery and retired-route manifest.
- `search-epoch-pins`: in-memory route/epoch pins, watermarks, renew/release and restart semantics.
- `search-index-reclaimer`: exact-ID ordinary reclaim after visibility and pin guards; no security-purge
  authority.
- `eliot-searchd` W3 re-entry: compose the accepted W2 daemon handoff with the qualified indexed profile.

### W3 exit

Pin one exact Qdrant server/client/profile set. Prove artifact identity, process containment,
capabilities, schema, lexical vectors, point collision safety, publication failpoints, uncommitted epoch
invisibility, exact readback, pin cleanup and exact retired-point reclaim. Publish W3 receipt; G2 remains
open until the query product exists.

## W4 — Useful baseline query product

### Package implementation

- `search-access`: compile non-widening eligibility and safe legs; apply live deny/currentness before
  candidate generation, IDF, counts, traces and emission.
- `search-query-planner`: validate closed recipes, bind immutable request snapshots and compile finite
  server-owned plans and budgets.
- `search-retrieval-executor`: bounded scheduler lanes, direct/index legs, pin acquisition, deterministic
  fusion, contamination handling, cancellation and cleanup.
- `search-candidate-validator`: exact retained-revision readback, span/digest validation and live
  access/currentness/purge recheck before emission.
- `search-handles`: opaque request/binding/source/revision-bound handles with TTL, quotas,
  reauthorization and cleanup.
- `search-result-projector`: deterministic compact cards, exact source references, coverage/gaps,
  truncation and disclosure limits.
- `search-continuation`: opaque plan/access/route/epoch-bound continuation state with exact resume,
  reauthorization, expiry and restart cleanup.
- `search-eval`: raw direct/grep baseline capture, deterministic case/evidence records and measurement
  seams without self-acceptance.
- `eliot-searchd` W4 re-entry: compose authenticated
  access → plan → execute → validate → project → handle/continuation.

### W4 exit and G2

Prove bounded deterministic lexical cards, access/IDF noninterference, stale and inaccessible candidate
removal, source-backed evidence, truthful partial/coverage state, handle/continuation reauthorization,
cancellation cleanup and no hot-query control writes. An accepted W4/G2 is the first useful baseline
product, but it is not yet release-ready.

## W5 — Current workspace and qualified Rust structure

- `search-source-reconcile`: treat watcher/USN events as hints, open observation gaps on overflow/reset,
  run bounded authoritative inventory and close a gap only after guarded complete commit.
- `search-overlay`: keep unsaved snapshots memory-only, apply `unsaved > saved > published`, shadow stale
  base populations before retrieval/IDF/counts, transition through explicit save admission and
  invalidate on restart.
- `search-code-enricher`: qualify one exact tolerant Rust parser profile; preserve `cfg` predicates,
  anchors and assurance; forbid Cargo/rustc/build scripts/macros/LSP/shell/network execution.
- `eliot-searchd` W5 re-entry: publish current-workspace capability only when filesystem, saved revision,
  buffer and projection currentness axes justify it.

Publish W5 only after overflow, restart, save, malformed syntax, Unicode/encoding, unsaved sink audit and
currentness-denial probes pass.

## W6 — Comparison and exact proof

- `search-subject-resolver`: deterministic resolution ladder, bounded candidates, explicit ambiguity,
  source-backed lineage and drift revalidation.
- `search-comparator`: independent evidence legs, lineage/fork collapse, `cfg` applicability,
  descriptive differences, recommended reading order and truthful coverage; never emit a hidden
  normative winner.
- `search-exact`: qualify exact regex/structural profiles, freeze an authoritative denominator, execute
  one bounded outcome per item, checkpoint/resume and revalidate drift, access and unsaved state.
- `eliot-searchd` W6 re-entry: expose comparison and exact recipes only through accepted W5 state and
  existing authenticated provider paths.

G3 requires zero false complete-negative claims, honest ambiguity/coverage, exact denominator closure and
correct failure behavior for drift, unreadable items, cancellation and security changes.

## W7 — Security and lifecycle hardening

W7 reopens only the packages with lifecycle responsibilities:

- `search-retention`: policy roots, mark/sweep coordination, purge planning, tombstones, restore
  quarantine and truthful receipts;
- `search-revision-store`: exact object deletion, purge tombstones, retention leases and
  non-resurrection;
- `search-access`: immediate restrictive barriers and active-request contamination;
- `search-handles`: durable handle invalidation and sweep;
- `search-continuation`: continuation invalidation and pin/window cleanup;
- `search-candidate-validator`: final lifecycle fence before emission;
- `search-publication`: restrictive lifecycle/publication barrier ordering;
- `search-index-reclaimer`: exact ordinary-reclaim versus purge behavior and recovery;
- `eliot-searchd`: recovery ordering, drain, shutdown and receipt composition.

Required fault matrices include crash at every durable transition, mid-request revoke/purge, restart
during mark/sweep, restore of tombstoned data, delayed Qdrant deletion, leaked handle/pin checks and
idempotent replay. W7 produces the separate `W7_LIFECYCLE` receipt; it is not silently folded into G3.

## W8 — Generic client edge and standalone product interface

- harden `search-provider-protocol` for full authenticated binding, replay defense, capability filtering,
  limits, cancellation and handle expansion;
- re-enter `eliot-searchd` with the accepted W7 daemon handoff and generic-edge composition;
- complete the standalone `eliot-search` client without direct store authority;
- implement `search-eliot-adapter` and `search-research-export-adapter` only as disabled leaf adapters
  with one-way typed mapping and no reverse authority.

G4 requires an authenticated generic request → plan → candidate → result round trip, capability
filtering, replay/cancel/resource tests and live handle reauthorization. Optional leaf adapters do not
block the baseline product unless explicitly enabled.

## W9 — Product Pulse and release candidate

W9 reopens only `search-eval`. Before any candidate result is inspected, the integration owner must
freeze:

- one exact Windows environment and hardware/resource profile;
- immutable control corpus and oracle ownership;
- exact DIRECT/LEXICAL/CODE candidate revision and artifact digests;
- exact A/B baselines;
- randomized paired schedule and minimum sample counts;
- primary/secondary metrics, non-inferiority margins, practical effects, resource regression limits and
  hard blockers;
- fault, protocol stress, leakage and recovery matrices;
- independent reviewer identity and acceptance policy digest.

Candidate architecture targets currently include:

```text
warm exact keyword/navigation p95          ≤ 100 ms
warm single-scope lexical p95              ≤ 200 ms
warm cross-repository comparison p95       ≤ 700 ms
first useful progressive card              ≤ 300 ms
minimum measured samples per percentile    30
```

Hard blockers cannot be averaged away:

```text
false complete-negative claims             0
stale/currentness leakage                   0
access leakage                              0
secret/source/query/token/path leakage      0
protocol resource leaks                     0
recovery correctness                        100% of required fault cells
```

G5 requires raw quality, latency/resource, fault/recovery, protocol stress and leakage evidence plus an
explicit Product Pulse verdict and independent review. Only accepted G5 produces a release candidate.

## W10 — Optional depth after baseline acceptance

Do not implement or enable W10 merely to make the baseline product look complete. Select at most one
candidate per dedicated ADR:

```text
model: rerank-only, dense or multivector
document: one exact isolated no-execute provider profile
scale: one exact measured Qdrant topology migration
```

Each candidate requires exact artifact/license qualification, explicit feature/config/binding
authorization, pre-registered incremental benefit, baseline non-regression, complete removal and
migration/rollback evidence where applicable. Failure or removal must restore accepted P15 behavior
before optional physical reclaim.

## Cross-cutting implementation requirements

### Error and outcome model

- every public operation returns typed reasons and retryability;
- timeout/cancellation after a possible external mutation is `OUTCOME_UNKNOWN` until exact readback;
- partial, degraded, unavailable, quarantined and cancelled remain distinct from success;
- public errors are bounded and redacted but retain actionable correlation/recovery identifiers;
- panic is not a control-flow or recovery mechanism.

### Concurrency and resource bounds

- every queue, batch, page, stream, request, candidate set, source read, byte count, handle,
  continuation, pin and retry loop is finite;
- cancellation is propagated across all legs and releases guards, pins, temporary files and worker
  requests;
- foreground requests cannot be starved by background rebuild/reclaim work;
- background work is resumable, rate-limited and observable;
- no global mutable state outside its declared owner.

### Security

- local endpoints require authenticated binding; local machine presence is not authorization;
- source admission and final-handle containment defend against path traversal, symlinks, junctions and
  reparse races;
- ACLs, secret references and data-root ownership fail closed;
- access/currentness/purge checks precede every influence and emission boundary;
- unsaved bytes never enter disk, backups, crash dumps, telemetry, provider caches, evaluation or
  training inputs;
- optional providers receive only bounded minimized inputs and can never create source evidence;
- secure erase is never claimed without executed evidence.

### Persistence, migration and recovery

For every redb/CAS/Qdrant state transition define:

```text
operation identity
precondition and guard
durable intent
external mutation
readback
control commit
crash points
recovery decision
idempotent replay
quarantine condition
rollback or forward-completion path
```

Schema migrations must be versioned, restart-safe and tested from every supported previous release.
Backups and restore must preserve authority, residency and tombstone semantics; restore cannot resurrect
purged data.

### Observability and diagnostics

Implement structured, bounded and redacted events for:

- startup phase and readiness reason;
- request/plan/leg/candidate correlation;
- queue and resource pressure;
- currentness and observation gaps;
- publication generation/epoch and recovery state;
- handle/continuation/pin lifecycle;
- purge/reclaim/restore state;
- worker and Qdrant process identity;
- configuration snapshot and pending action set.

Diagnostics must never include source bodies, query text by default, secrets, opaque token contents,
foreign membership or unrestricted paths. `doctor` must report exact failed prerequisites and remediation
without opening stores outside daemon authority.

### Performance

- preserve the W9 metric definitions and timers from the beginning, not as a late instrumentation patch;
- maintain deterministic warm/cold separation and exact sample denominators;
- benchmark direct, lexical, code, comparison, exact and failure paths independently;
- record CPU, working set, private/commit bytes, disk I/O, queue depth and background duty cycle;
- optimize only after correctness and source-backed evidence are preserved;
- no cache may widen access, hide staleness or survive a scope/revision/profile mismatch.

### Testing

Every package must include, as applicable:

```text
unit tests for pure rules
golden serialization/vector/manifest fixtures
property tests for bounds, ordering, identity and non-widening
negative tests for malformed/unknown/oversized input
fake-port contract tests
fault injection around every durable/external mutation boundary
cancellation/deadline tests
restart/recovery tests
content and metadata disclosure tests
Windows path/ACL/process tests
stage qualification fixtures
```

A successful compile or unit suite is never sufficient for a gate.

### Packaging, installation and service operation

Before G5, provide an integration-owned Windows release bundle containing:

- versioned `eliot-searchd` and `eliot-search` binaries;
- exact dependency/license/SBOM and checksum manifests;
- configuration schema and safe DIRECT example;
- an exact qualified Qdrant artifact or an explicit separately installed artifact contract;
- install, service registration, start/stop, upgrade, rollback and uninstall procedures;
- data-root ACL creation and ownership verification;
- log/diagnostic collection with redaction;
- backup/restore and rebuild procedures;
- removal that leaves user source data untouched and handles retained product data according to policy.

The installer must not download or silently upgrade runtime artifacts. Upgrade rehearsals must cover
process drain, schema migration, Qdrant compatibility, rollback and interrupted installation.

### User and operator documentation

Ship:

- minimal quick start for DIRECT and qualified indexed mode;
- configuration reference generated from the twenty section owners;
- CLI and protocol compatibility reference;
- source admission and privacy model;
- rebuild, recovery, backup, restore and purge procedures;
- troubleshooting for ownership, Qdrant, parser, currentness and capability failures;
- explicit limitations and non-goals;
- version/support policy and artifact verification instructions.

## Required PR sequence

The implementation program should use this pattern:

```text
integration bootstrap PR
→ one package implementation PR per dependency-ready package
→ independent package review
→ immutable public package handoff
→ stage composition/re-entry PRs
→ stage qualification evidence PR
→ independent wave/gate acceptance PR
→ launch-state advancement PR
```

The first concrete sequence is:

1. integration bootstrap: toolchain, dependency policy, `Cargo.lock`, test harness and artifact layout;
2. `search-contracts` implementation;
3. independent `search-contracts` review and accepted handoff;
4. `search-domain` and `search-ports` in parallel;
5. W0/G0 evidence and acceptance;
6. launch-state advancement to W1;
7. dependency-safe W1 package PRs;
8. continue strictly through W2–W9.

Do not open all package implementations concurrently. A future package may have complete documentation
and still be blocked by launch state or missing accepted handoffs.

## Merge requirements for implementation PRs

A package implementation PR is mergeable only when:

- its issued ticket, context digest and lease are exact and current;
- diff is contained in the package write scope;
- Cargo dependencies equal the registry and use only accepted public APIs;
- all routed operations and logical modules for the package are implemented or explicitly unavailable
  under the ticket;
- public types/reasons/ports match accepted digests;
- tests begin from contract failures and cover negative/fault/cancellation paths;
- handwritten line limits are respected;
- no secret/content/vendor/internal implementation leaks cross the boundary;
- package-specific and structural validators pass with raw output;
- no unresolved contract challenge remains;
- writer has not self-reviewed or modified control-plane authority.

## Release go/no-go

The baseline product may be released only when all are true:

```text
G0 accepted
G1 accepted
G2 accepted
G3 accepted
W7_LIFECYCLE accepted
G4 accepted
G5 accepted with independent Product Pulse review
all mandatory packages have accepted handoffs
all active leases and unresolved submissions are closed
Windows install/upgrade/restart/recovery/rollback/uninstall rehearsal passes
hard safety/correctness blocker counts are zero
accepted configuration, protocol, schema, artifact and data-layout digests are pinned
support, recovery and security documentation ships with the artifact
```

W10/G6 is not a baseline release requirement.

## Explicit non-goals of this plan

This plan does not:

- authorize a writer or mutate launch state;
- choose exact third-party versions or optional providers;
- accept an artifact, package, wave or gate;
- permit architecture-master reads by ordinary package writers;
- allow package writers to edit shared registries, evidence or release state;
- require optional model/document/scale depth for the first reliable product;
- treat static coverage, generated maps or green unit tests as runtime evidence.
