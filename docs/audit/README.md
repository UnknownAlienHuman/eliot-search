# ELIOT Search audit entrypoint

**Current preparation snapshot:** `bf108e3ffefa6cea90ac2b5f78914c736529b60f`  
**Coordinator:** `#97`  
**Product/release ready:** no  
**One-manager Wave 1:** authorized  
**Wave 2 preparation:** complete  
**Wave 2 coding:** blocked until `#237` merges and exact `WAVE2_BASE_SHA` is published

## Read this first

```text
AGENT_LAUNCH_GATE_2026-10-09.md
→ WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md
→ WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md
→ exact active issue and package AGENTS.md
```

For donor decisions:

```text
DONOR_VERIFICATION_REGISTER_2026-10-09.md       # Wave 1 / canonical / Unicode decision
WAVE2_DONOR_ACCEPTANCE_2026-10-09.md            # Wave 2 exact mechanisms/corrections
WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md    # exact one-manager path/root exceptions
```

## Technical audit versus execution authority

`ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md` remains the consolidated technical finding register through F68 and the subsystem diagnosis. Its older audited SHA and its former “two managers / disjoint foundations” launch wording are historical.

Current execution authority is:

1. root `AGENTS.md`;
2. Architecture Part I and accepted ADRs;
3. this directory’s current launch gate and Wave packets;
4. coordinator `#97` latest accepted-base comment;
5. exact current issue and nearest package instructions;
6. current source reality.

A historical audit section, packet PR, branch or source-donor snapshot cannot start work or widen file ownership.

## Current one-manager topology

```text
Wave 1
  #237 canonical/digest foundation
  #253 Unicode full-fold decision

Wave 2 after #237
  #258 → #250 → #256 → #257 → #266 → #235
  → #238 → #241 → #246 → #252
  → #226 Markdown → #226 JATS
```

The manager owns one writer worktree and advances one merged PR at a time. Five to ten subagents are read/research/review only.

## Key corrections already applied

- one manager replaces the former two-manager model;
- `search-contracts` remains the sole canonical codec/digest contract owner;
- `#258` is first in Wave 2;
- `#226` follows UnitSet v3 `#257`;
- exact `pulldown-cmark 0.13.1` has no `std` feature;
- `cargo_metadata::MetadataCommand::exec()` is not a bounded process runner;
- quick-xml reader positions are not automatically exact semantic source ranges;
- package/root exceptions are exact issue rows, not general permission;
- closed donor branches are read-only source/fixture archives, never merge bases.

## Status and evidence

`docs/product/PACKAGE_STATUS.toml` separates contract, source, product-path, check/Clippy evidence, native qualification and public enablement. Source presence or a detailed issue never implies integration or qualification.

No audit/launch document proves:

- exact-head Cargo or strict Clippy;
- native Windows owner/pipe/process/secret behavior;
- real selected Qdrant generation and recovery;
- installed end-to-end product;
- scale/resource/disclosure qualification;
- release package acceptance.

Those claims require the exact owner and `#215/#240/#140` evidence chain.

## Historical material

Older dated audits, `swarm/**`, `docs/handoff/**`, `docs/execution/**`, packet-only PRs and Architecture Part II remain useful for archaeology and obligation tracing only. They are not implementation authorization.