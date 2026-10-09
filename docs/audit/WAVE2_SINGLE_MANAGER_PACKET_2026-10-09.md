# Wave 2 single-manager execution packet — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Preparation base:** `5d0435a55db8120d629d14d5167db6d737ea90c2`  
**Coding base:** the exact post-`#237` `main` SHA published by coordinator `#97`; it does not exist yet.  
**Topology:** one manager, one writer worktree, eight read/research/review subagents.  
**Product/release qualification:** not performed.

## Launch verdict

```text
Wave 2 preparation:                         READY
Wave 2 coding before #237 merge:            FORBIDDEN
One manager + 8 read/research subagents:     READY FOR PRELAUNCH REVIEW
Independent writer branches/worktrees:      FORBIDDEN
Wave 2 coding base:                          UNPUBLISHED UNTIL #237 MERGES
```

Wave 2 is not eleven agents editing independent branches. The same manager owns one worktree and advances a serialized chain of small PRs. Subagents inspect exact source, donors and candidate diffs; they never write repository state, resolve `Cargo.lock`, push or merge.

## Mandatory authority order

Every participant reads, in order:

1. [root `AGENTS.md`](../../AGENTS.md);
2. [Architecture entrypoint](../architecture/README.md) and normative Part I;
3. [ADR-0005](../adr/0005-standalone-search-product-and-controller-boundary.md);
4. [ADR-0006](../adr/0006-agent-analysis-framework-product-scope.md);
5. [package status matrix](../product/PACKAGE_STATUS.toml);
6. [Wave 1 packet](./WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md);
7. [Wave 2 donor register](./WAVE2_DONOR_ACCEPTANCE_2026-10-09.md);
8. this packet;
9. coordinator `#97` latest accepted-base comment;
10. the exact current issue and nearest package `AGENTS.md`/`FUNCTIONS.md`/README;
11. current source on the published base;
12. a closed donor branch only for the exact files named by the issue.

Historical swarm tickets, packet PR branches and closed donor branches are never working bases.

## Why Wave 2 is serialized

The original “ready after #237” list hid four overlap classes:

1. `#258` extends `search-contracts`, the same shared owner changed by `#237`.
2. `#238`, `#241`, `#246`, `#250`, `#252` and `#226` add or change root dependency pins and `Cargo.lock`.
3. `#257` and `#226` both own `search-unitizer`; document profiles cannot define a second temporary UnitSet.
4. `#266` crosses `search-runtime-owner`, daemon composition and legacy DIRECT open paths.

One manager removes merge conflicts but does not remove semantic dependencies. Every slice starts from the latest merged `main`, not from the initial Wave 2 base.

# 1. Exact merge order

## Batch 2A — shared contracts and foundational owners

```text
A1  #258 shared S9.5/S10.3/epoch contract in search-contracts
A2  #250 cargo_metadata + PACKAGE_STATUS tooling guard
A3  #256 exact S11 point identity
A4  #257 authoritative UnitSet v3
A5  #266 one typed data-root admission/open owner
A6  #235 one reusable provider client
```

Rationale:

- `#258` lands first because every indexed consumer must compile against one contract surface.
- `#250` lands early so subsequent workspace/dependency/status changes are checked from Cargo facts rather than handwritten inference.
- `#256`, `#257`, `#266` then remove critical legacy authority without new parser frameworks.
- `#235` extracts the existing typed client only after the shared contract/tooling base is stable.

## Batch 2B — mature donor cutovers

```text
B1  #238 maintained TOML parser + typed config visitor
B2  #241 bounded GlobSet admission profile
B3  #246 memchr/Aho-Corasick literal engine cutover
B4  #252 code_identifiers@1 with XID + NFC
```

`#250` already owns the tooling cutover, so it is not repeated here.

## Batch 2C — document profiles

```text
C1  #226 Markdown profile only
C2  #226 JATS profile only
```

`#226` is blocked by `#257`. Markdown and JATS are separate logical commits and preferably separate PRs/review points. No document profile may invent a provisional manifest or UnitSet.

## Explicitly not Wave 2

```text
#247–#249 multi-literal/regex/exact integration
#251 syn source lints
#254 prose_words@1 implementation
#259–#264 indexed planner/publication/bridge/product proof
#267–#272 safe read/control/ingestion/currentness/scope
#116 CLI invocation/native endpoint
#130/#219 optional LSP/MCP
all destructive lifecycle and release work
```

Detailed successors do not start automatically.

# 2. One-manager branch protocol

After `#237` merges, coordinator `#97` publishes `WAVE2_BASE_SHA`.

The manager creates one worktree:

```text
worktree: ../eliot-search-manager-wave2
first branch: manager/258-index-contract
base: exact WAVE2_BASE_SHA
```

For every slice:

```text
latest accepted main
→ fresh issue branch in the same manager worktree
→ subagent preflight reports
→ code first
→ minimal locked check + strict Clippy
→ independent SA-08 review
→ PR
→ merge
→ publish new main SHA
→ next branch
```

No stacked unmerged PRs. No shared dirty worktree. No force-push over another branch. The manager is the sole root dependency and `Cargo.lock` integrator.

# 3. Current source entrypoints

## `#258` — shared indexed contract

Read:

- `crates/search-contracts/AGENTS.md`;
- `crates/search-contracts/src/{canonical,ids,schema,source}.rs`;
- `docs/audit/INDEXED_SPINE_PORT_MAP_2026-10-09.md`;
- donor snapshots `#209` (`e5c14cde...`) and `#200` (`615d64f...`) only for named S9.5/S10.3 field/filter fixtures.

Output: one provider-neutral 20-field payload, 19-field baseline index plan, closed eligibility population and generation-local `0..=2^53` epoch domain. No Qdrant SDK type.

## `#250` — Cargo graph/status tooling

Read:

- `xtask/src/qdrant_boundary.rs` and `qdrant_boundary/{manifests,filesystem,source}.rs`;
- `docs/product/PACKAGE_STATUS.toml`;
- exact `cargo_metadata 0.23.1` source.

Do not call `MetadataCommand::exec()` and claim bounded execution: exact source uses `Command::output()` and captures complete stdout/stderr before parsing. Use `MetadataCommand::cargo_command()` only to construct the official command, then one bounded xtask subprocess adapter with byte/deadline/status limits, or return `BLOCK` if the existing tooling boundary cannot support that safely. Parse only capped successful output.

## `#256` — S11 point identity

Read:

- current `search-point-identity/src/lib.rs`;
- closed donor `#207` files `key.rs`, `collision.rs`, `uuid.rs`, tests;
- accepted `#237` canonical/digest APIs.

Port model and fixtures, not donor codec/crypto. Delete mutable identity registry and legacy FNV production minting.

## `#257` — UnitSet v3

Read:

- `search-unitizer/src/manifest/**`, `layout**`, current tests;
- contract `UnitOccurrence`, `UnitId`, `UnitKind`, `NativeAnchor` definitions;
- materializer representation/coordinate/loss identities.

One verified complete ordered UnitSet replaces caller list/count/digest authority. V2 is legacy decode/rebuild only.

## `#266` — root admission owner

Read:

- `search-runtime-owner/src/{identity,lease,state,supervisor,transition}.rs`;
- daemon `owner_composition/**`;
- `direct_store/store.rs::DirectStore::open` and every caller;
- control owner/quarantine readback paths.

Separate inspect-existing, mutate-existing, initialize-new and named-recovery capabilities. Read-only paths create nothing.

## `#235` — provider client

Read:

- `bins/eliot-search/src/provider_client/core.rs`;
- `provider_client/typed.rs`;
- `typed/{io,local,native,state}.rs`;
- `search-provider-protocol` framing/pairing/session contracts.

Move only the typed canonical path into `crates/search-provider-client`. Keep legacy loopback/core harness code out of the new public library. Delete CLI production copies after cutover.

## `#238` — TOML/config

Read current `search-config/{document,fingerprint,limits,registry,merge}.rs` and exact donor source. Syntax belongs to TOML; ELIOT retains closed descriptors, precedence, reset/override and secret semantics.

## `#241` — admission globs

Read the current 139-KiB `search-source-admission/src/lib.rs` and exact `globset` source. Split only by real model/profile/matcher/evaluation/receipt ownership while deleting the copied SHA/writer and inert pattern path.

## `#246` — literal engine

Read `search-exact/src/literal.rs` and preserve all current semantics: aggregate input limits before scan, chunk-boundary transparency, overlapping matches, ASCII-only folding and one-extra-match truncation proof.

## `#252` — code identifiers

Read `search-lexical/src/{lib,frozen,root,scoring}.rs` and sparse profile code. Add one internally identified XID+NFC profile; do not mix unresolved full case folding or reinterpret old vectors.

## `#226` — Markdown/JATS

Read current materializer/unitizer APIs before creating modules. The current tree has no accepted generic `profiles/` framework. Add only cohesive modules that reuse actual representation, map, assurance and UnitSet v3 owners.

# 4. Subagent assignments

The manager may run eight subagents concurrently for preflight and then reuse them for PR review. They are read-only.

## W2-SA01 — authority and package-instruction audit

Return `W2_AUTHORITY_MAP`:

```text
issue
owned paths
nearest AGENTS constraints
exact Wave-2 exception
prerequisites
forbidden neighbours
stale base/path/issue references
ACCEPT / AMEND / BLOCK
```

Block if two current tasks own the same facade or if a package instruction still forbids the required exact work.

## W2-SA02 — dependency and lockfile acceptance

For every external donor return:

```text
crate/version/tag/commit/checksum
license/data license
MSRV
normal/build/dev closure
enabled/disabled features
advisories/maintenance
runtime network/download/default behavior
API facts used by the task
existing lock reuse or new resolution
ACCEPT / REJECT / BLOCK
```

No manifest change occurs until the manager verifies this record.

## W2-SA03 — indexed contract/identity port map

Audit `#258` and `#256` against current contracts and donor snapshots. Return exact fields/domains/goldens to retain, duplicate codec/crypto/state to reject, compatibility/rebuild rule and deletion list.

## W2-SA04 — UnitSet/document preparation map

Audit `#257` and `#226`. Return current materializer/unitizer APIs, exact missing v3 fields, source-range/coordinate capabilities, donor API facts, proposed modules and proof that document profiles consume—not redefine—the UnitSet.

## W2-SA05 — root/client composition map

Audit `#266` and `#235`. Return every real `DirectStore::open`/root-admission caller, service versus one-shot bypass, typed client code to move, legacy harness code to retain/delete and exact shutdown/cancellation ownership.

## W2-SA06 — config/admission donor red-team

Audit `#238` and `#241` from exact release source. Verify hidden defaults, recursion/resource behavior, typed visitor/candidate bytes, diagnostic disclosure, synchronous compilation limitations and all old code to delete.

## W2-SA07 — exact/tooling/lexical donor red-team

Audit `#246`, `#250`, `#252`. Verify overlapping/case semantics, chunk adapter, metadata subprocess/output bounds, XID/Unicode version/NFC and all feature/transitive closure.

## W2-SA08 — independent final PR review

For every candidate PR return only blocking findings or `APPROVE_SOURCE`:

- issue scope and prerequisites;
- no second authority or temporary facade;
- donor versions/features equal accepted record;
- old product path actually deleted or explicitly legacy read-only;
- no canonical/profile bytes silently changed;
- no unbounded allocation/process output before limits;
- exact locked check and strict Clippy output;
- next owner remains blocked correctly.

`APPROVE_SOURCE` is not product qualification.

# 5. Corrected donor decisions

The normative details are in `WAVE2_DONOR_ACCEPTANCE_2026-10-09.md`. Key corrections:

- `pulldown-cmark 0.13.1` has no `std` feature in the exact tag; use `default-features = false` with no nonexistent feature.
- `cargo_metadata 0.23.1::MetadataCommand::exec()` captures complete subprocess output; Wave 2 must add a bounded execution wrapper rather than claiming the donor is bounded.
- `quick-xml` buffer/error positions are not automatically exact semantic element/text ranges; derive and verify ranges against retained raw XML with trimming/encoding transformations disabled.
- `#226` follows `#257`; it cannot define a provisional UnitSet.
- `#258` is first after `#237`; no other Wave-2 branch may concurrently edit `search-contracts`.

# 6. Per-slice development gates

Each issue retains its package commands. Minimum policy:

```text
cargo +1.98.0 check --locked -p <affected packages> --all-targets
cargo +1.98.0 clippy --locked -p <affected packages> --all-targets -- -D warnings
```

Run the smallest focused causal fixtures only when the change requires immediate proof. Broad/full/product/native matrices remain deferred under project policy. Report exact toolchain, platform, command, exit and nonzero target/fixture names; never convert compilation into qualification.

Before merging a dependency PR also run:

```text
cargo +1.98.0 metadata --locked --offline --format-version 1
```

and the accepted `#250` status/Qdrant-boundary command once `#250` is merged.

# 7. Universal completion report

```text
issue and batch/order
base/result SHA
one worktree/branch history
subagents used and dispositions
exact files/packages changed
root pins/Cargo.lock delta
donor source/version/checksum/license/MSRV/features/advisories
source donor files ported and rejected
old code deleted/legacy-read-only path
profile/generation/migration impact
locked check outcome
strict Clippy outcome
focused execution status
next blocked issue and required main SHA
no qualification claim beyond evidence
```

# 8. Stop conditions

Stop instead of improvising if:

- `#237` has not merged or coordinator has not published `WAVE2_BASE_SHA`;
- the current branch is not based on latest accepted `main`;
- a package instruction still conflicts with the exact slice;
- a donor version/API/feature differs from the acceptance record;
- canonical/profile bytes would change without a new profile/generation/migration;
- another package/facade must be edited outside the exact issue exception;
- a second codec, client, root owner, UnitSet, schema table, parser or catalog would be introduced;
- donor public types would cross ELIOT boundaries;
- subprocess/parser/matcher allocation occurs before accepted ceilings;
- compatibility code remains able to mint current product identities;
- a subagent writes repository state or self-approves;
- a broad test/product/release PASS is requested from a package source slice.

## Readiness statement

Wave 2 is now specified for one manager and eight bounded subagents. It is **prepared, not launched**. Coding starts only after `#237` merges and `#97` publishes the exact base and first branch. The packet removes parallel-writer ambiguity, fixes donor/API errors and defines a deterministic merge path through all eleven Wave-2 slices.