# Swarm readiness audit — 2026-08-29

## Verdict

The repository is structurally ready to launch **P00/W0 only**. It is not ready to launch all packages
simultaneously and it is not an implemented product.

The hardened workspace contains **39 library packages and 4 binaries**. Every package has one bounded
assignment. Five support packages were added because the first decomposition still left load-bearing
security or lifecycle state without a single owner.

## First-pass findings already closed

- broad source/index/query family crates were replaced by bounded capability packages;
- package-local assignments now include primitives, operations, invariants, typed failures and tests;
- `swarm/launch-state.toml` is the only launch authority;
- daemon composition is progressive rather than one all-context task;
- root/toolchain/lockfile/CI/generated-schema changes belong to the integration owner;
- optional model/document depth is machine-blocked until accepted P15 evidence and an ADR;
- split review occurs before 8,500 lines and 10,000 hand-written Rust lines is a hard stop.

## Second-pass findings closed

| ID | Severity | Finding | Closure |
|---|---:|---|---|
| F-01 | Blocker | `expand_handle@1`, durable eligibility, TTL, revocation and expansion authorization had no mutable-state owner. | Added `search-handles`; projector selects subjects, continuation owns only continuation state. |
| F-02 | Blocker | C17 produced a watermark but no package owned ordinary exact retired-point deletion. | Added `search-index-reclaimer`; deletion requires a committed exact manifest and safe pin watermark. |
| F-03 | Major | Qdrant process qualification/lifecycle and vendor data-plane calls shared one package and daemon responsibility. | Added `search-qdrant-supervisor`; `search-qdrant-bridge` is data-plane only. |
| F-04 | Major | OS-bound Qdrant/provider credentials had no reusable security owner. | Added `search-os-secrets` with opaque references and no public plaintext contract. |
| F-05 | Major | Source-admission decisions were duplicated by registry and safe reader. | Added pure `search-source-admission`; registry verifies receipts and safe reader performs I/O only. |
| F-06 | Blocker | Query/lifecycle packages depended directly on concrete Qdrant, redb or revision-store adapters. | Orchestration consumes ports; only `eliot-searchd` constructs concrete adapters. |
| F-07 | Major | Ordinary point reclaim, CAS retention and security purge could collapse into one deletion path. | Reclaimer, revision/CAS lifecycle and purge receipts now have separate owners and acceptance semantics. |
| F-08 | Major | The first hardening draft still linked the Qdrant process adapter directly to the concrete OS-secret adapter. | Supervisor now accepts a bounded secret lease; daemon composes both ports and neither adapter opens the other. |

## Deliberate non-splits

- `search-publication` retains commit and crash recovery as one linearizable state machine.
- `search-exact` retains compile/execute because one denominator/completeness owner is required.
- `search-retention` retains CAS mark/sweep, purge and restore quarantine while one monotonic lifecycle
  policy dominates them.
- baseline filesystem/Git stable reads remain in `search-safe-reader` until dependencies or measured size
  create an actual backend boundary.

These packages still carry earlier split triggers; no forwarding or crate-per-function shells were
introduced.

## Mechanical acceptance required before merge

- Cargo members, workspace dependency paths and `swarm/crates.toml` package paths are identical;
- exactly 43 package assignments exist;
- every registry dependency names an existing package and the graph is acyclic;
- ordinary packages depend only on the same or an earlier first-launch wave;
- `eliot-searchd` is the sole exempt progressive-composition package and later dependencies are
  feature-gated;
- new packages contain only boundary placeholders, not business implementation;
- query/lifecycle Cargo manifests contain no concrete redb/Qdrant/process adapter edge;
- launch state remains P00/W0 and optional depth remains blocked.

## Still deliberately unresolved

These require real implementation or executed qualification and are not replaced by prose:

- exact stable Rust toolchain and `Cargo.lock` for the selected Windows dependency set;
- exact external dependency versions, sources and license evidence;
- exact Qdrant server/client patch and executable SHA-256;
- Windows process, ACL, secret-store and crash/fault evidence;
- redb/CAS/Qdrant runtime correctness;
- latency, resource, access-noninterference and Product Pulse results.

## Launch recommendation

1. Assign `search-contracts` only.
2. Accept its API/schema digest and contract fixtures.
3. Assign `search-domain` against that immutable handoff.
4. Let the integration owner pin toolchain/dependencies, generate `Cargo.lock` and execute P00 policy
   checks.
5. Publish a W0 receipt before advancing `active_wave`.
