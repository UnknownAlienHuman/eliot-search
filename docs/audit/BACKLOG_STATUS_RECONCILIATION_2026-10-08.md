# Backlog and status reconciliation — 2026-10-08

**Audited source:** `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`  
**Coordinator:** #97  
**Documentation owner:** #222

## Result

The early P00/P01/P02/W0/W1/W2 issue wave duplicated package work that is already present or has a
new exact owner. Issues `#51–#89` were closed with `state_reason=not_planned` and an individual
successor comment. This means **superseded**, not completed or qualified.

Issue `#48` is closed as completed. Verification against the active `TYPE_REGISTRY.md` confirmed exact
named entries for `UtcTimestamp`, `MetadataKey` and `UnresolvedSource`; the stale handoff note, not the
normative registry, was incorrect.

## Successor map

| Area | Current owner |
|---|---|
| Canonical codec/digests | #237 |
| Configuration | #238 → #109 |
| Source admission | #241 → #110 |
| Source identity/registry/currentness | #110 → #128 → #218 |
| Provider client and product transport | #235 → #116 |
| Control redb | #106 → #108 |
| Runtime/data root/native boundaries | #100/#104/#105/#120 |
| Secrets | #115/#120 |
| Safe reader/Git | #104/#129 |
| Revision/retention/restore | #111/#134/#243 → #244 → #245 |
| Materialization/documents | #112/#113/#216/#226/#227 |
| Installed qualification | #240 → #215 |

## Closed issues

```text
#51 #53 #54 #55 #56 #59 #60 #61 #63 #64 #65
#68 #69 #70 #71 #72 #73 #74 #75 #76 #77 #78
#79 #80 #81 #82 #83 #84 #85 #86 #87 #88 #89
```

Other early issues already closed before this reconciliation are not reopened.

## Documentation contradiction resolved by this branch

At the audited `main` revision, 26 active README files still contained the obsolete sentence
`behavior is intentionally unimplemented`, including domain, ports, query, source, lifecycle,
Qdrant and edge packages with substantive source.

This branch replaces all 26 active false scaffold claims with evidence-bounded status, preserves
`Owns` / `Must not own` boundaries, names the selected current owner or successor, and links every
workspace member to `docs/product/PACKAGE_STATUS.toml`. Selected obsolete point-identity,
projection-planner and custom code-enricher paths are marked `legacy_profile`; optional workers and
adapters remain disabled rather than being advertised as available.

Historical scaffold documents may retain old wording only when clearly archived and non-authoritative.

## Central status matrix

`docs/product/PACKAGE_STATUS.toml` is the machine-readable inventory for the 48 active Cargo workspace
members. Schema v3 uses closed status values and structured references:

```text
execution_chain      ordered current implementation references
consumers            downstream users, not alternate owners
optional_followups   disabled/optional profiles, not baseline prerequisites
product_path         not_asserted | absent | partial | integrated | legacy_only
check_clippy_evidence
                     not_asserted | not_run | historical_pass | historical_fail |
                     exact_pass | exact_fail
disposition          active | legacy_profile | optional_disabled |
                     evaluation_only | tooling_only
```

The defaults record `contract_defined=true` and `source_present=true` only. They deliberately do not
assert product integration, exact-head check/Clippy evidence, native qualification or public enablement.
A later tooling guard under #138 must compare the matrix one-to-one with `cargo metadata --locked
--offline --no-deps` and reject stale README/status drift.

## Agent authority after cutover

```text
root/package AGENTS.md
→ #97 coordinator
→ exact current owner issue/PR
→ docs/product/PACKAGE_STATUS.toml
→ current source
→ package README responsibility notes
→ historical handoff/issues only for archaeology
```

A stale issue or historical README never authorizes a second package, parser, catalog, client,
protocol, watcher, restore journal, search index or release workflow.

## Evidence boundary

This reconciliation changes task authority and documentation status only. It does not assert a fresh
Cargo, Clippy, test, Windows, Qdrant, fault, scale or installed-product PASS. `SOURCE_PRESENT` remains
separate from `PRODUCT_PATH_INTEGRATED`, `LIVE_NATIVE_QUALIFIED` and
`PUBLIC_CAPABILITY_ENABLED`.
