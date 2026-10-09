# Agent launch gate — 2026-10-09

**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97)  
**Current main before this Wave-2 packet:** `5d0435a55db8120d629d14d5167db6d737ea90c2`  
**Topology:** one manager, one writer worktree, five to ten read/research/review subagents  
**Wave 1 packet:** [WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md](./WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md)  
**Wave 2 packet:** [WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md](./WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md)  
**Wave 2 donor register:** [WAVE2_DONOR_ACCEPTANCE_2026-10-09.md](./WAVE2_DONOR_ACCEPTANCE_2026-10-09.md)

## Readiness verdict

```text
Product/release:                       NOT READY
Independent multi-writer swarm:        FORBIDDEN
One manager + bounded subagents:        READY
Wave 1 implementation #237:            AUTHORIZED
Wave 1 research #253:                  AUTHORIZED
Wave 2 preparation:                    COMPLETE
Wave 2 coding:                         BLOCKED UNTIL #237 MERGES
Wave 2 coding base:                    NOT YET PUBLISHED
```

Subagents never write repository state. The manager alone owns the worktree, branches, manifests, root pins, `Cargo.lock`, PRs, merge decisions and gate claims.

## Start now — Wave 1

Use the exact `SINGLE_MANAGER_LAUNCH_SHA` published on coordinator `#97` and the Wave-1 packet.

```text
implementation: #237 canonical/digest foundation
research:       #253 Unicode full-case-fold decision
```

Required source gates after `#237` code:

```text
cargo +1.98.0 check --locked -p search-contracts -p xtask --all-targets
cargo +1.98.0 clippy --locked -p search-contracts -p xtask --all-targets -- -D warnings
```

Broad/full/product/native testing remains deferred under current project policy.

## Wave 2 launch transition

Wave 2 does not start merely because its issues are detailed. After `#237` merges:

1. coordinator `#97` publishes exact `WAVE2_BASE_SHA`;
2. package instructions and this packet are read from that SHA;
3. the same one-manager topology creates `manager/258-index-contract` from that exact base;
4. all subagents complete the Wave-2 preflight reports;
5. the manager advances one merged PR at a time.

No branch from the pre-`#237` main is accepted for Wave 2 coding.

## Wave 2 deterministic order

```text
Batch 2A
  #258 shared indexed contract
  #250 Cargo graph/status tooling
  #256 point identity
  #257 authoritative UnitSet v3
  #266 root admission/open owner
  #235 provider client

Batch 2B
  #238 typed TOML/config
  #241 GlobSet admission
  #246 literal matcher
  #252 code identifiers

Batch 2C
  #226 Markdown
  #226 JATS
```

`#226` is blocked by `#257`. Every dependency slice is merged by the same lockfile integrator. There are no stacked unmerged Wave-2 branches.

## Wave 2 subagents

The Wave-2 packet defines eight read-only assignments:

```text
W2-SA01 authority/package-instruction map
W2-SA02 dependency and lockfile acceptance
W2-SA03 indexed contract/identity port map
W2-SA04 UnitSet/document preparation map
W2-SA05 root/client composition map
W2-SA06 config/admission donor red-team
W2-SA07 exact/tooling/lexical donor red-team
W2-SA08 independent final PR review
```

The manager may run five to ten subagents, but the load-bearing assignment for the active issue and W2-SA08 are mandatory before merge.

## Important corrections now frozen

- `#258`, not `#256/#257`, is first because it exclusively extends `search-contracts` after `#237`.
- `#250` must not call `cargo_metadata::MetadataCommand::exec()` and call it bounded; exact source captures complete stdout/stderr through `Command::output()`.
- `pulldown-cmark 0.13.1` has no `std` feature in its exact tag; `#226` uses `default-features = false` with no nonexistent feature.
- `quick-xml` reader positions are not automatically exact semantic ranges; JATS must derive and verify mappings against retained raw bytes.
- document profiles consume the accepted UnitSet v3 from `#257`; they do not invent another manifest.
- current package `AGENTS.md` files receive narrow issue-specific Wave-2 exceptions; no general cross-package permission is created.

## Required per-slice gates

```text
cargo +1.98.0 check --locked -p <affected packages> --all-targets
cargo +1.98.0 clippy --locked -p <affected packages> --all-targets -- -D warnings
```

After `#250` merges, also run its accepted package-status/boundary validator for every subsequent workspace/dependency PR. Before a dependency merge:

```text
cargo +1.98.0 metadata --locked --offline --format-version 1
```

Report exact command, toolchain/platform, exit and affected targets. Compilation is not product qualification.

## Stop conditions

Stop and report instead of improvising if:

- the exact accepted base has not been published;
- a package instruction still conflicts with the current issue;
- a donor API/feature/version differs from the acceptance register;
- canonical/profile bytes change without a new profile/generation/migration;
- another package/facade must be edited outside the exact issue exception;
- a second codec/client/root owner/UnitSet/schema/parser/catalog appears;
- donor public types cross ELIOT boundaries;
- output/allocation/process capture occurs before ceilings;
- a compatibility path can still mint current product identities;
- a subagent writes or self-approves;
- a package PR is presented as Windows/Qdrant/product/release qualification.

## Evidence boundary

This gate proves only that Wave 1 can run and Wave 2 is fully mapped for a serialized one-manager continuation. It does not assert exact-head compilation, strict Clippy, Windows native behavior, Qdrant integration, installed-product readiness or release acceptance.