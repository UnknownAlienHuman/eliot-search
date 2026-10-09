# Agent launch gate — 2026-10-09

**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97)  
**Reviewed main before the current packet:** `e358d3c06d211b269efd501cd8d4002a260ba0a5`  
**Exact launch SHA authority:** latest `SINGLE_MANAGER_LAUNCH_SHA` comment on #97 after the packet PR is merged  
**Topology:** one manager, one writer worktree, five to ten read/research/review subagents  
**Selected packet:** [Wave 1 single-manager execution packet](./WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md)  
**Donor register:** [Donor verification register](./DONOR_VERIFICATION_REGISTER_2026-10-09.md)

## Readiness verdict

```text
Product/release execution:       NOT READY
Independent multi-writer swarm:  FORBIDDEN
One manager + 5–10 subagents:    READY AFTER PACKET MERGE/SHA PUBLICATION
Wave-1 code owner:               #237
Wave-1 parallel research:        #253
Wave-2 coding:                   BLOCKED BY #237 MERGE
```

Subagents do not write repository state. The single manager is the only actor allowed to edit the shared worktree, modify manifests or `Cargo.lock`, push branches, open implementation PRs, merge, or claim a gate result.

## Required entrypoint

Every participant starts with the exact reading order, subagent assignments, output schemas, source links, stop conditions and manager phases in the [single-manager packet](./WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md). The packet supersedes the former two-manager Wave-1 wording and the launch/topology sections of older audits or tracking PRs.

The manager must also read:

- [root `AGENTS.md`](../../AGENTS.md);
- [architecture entrypoint](../architecture/README.md);
- [normative Architecture Part I](../architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md);
- [ADR-0005](../adr/0005-standalone-search-product-and-controller-boundary.md);
- [ADR-0006](../adr/0006-agent-analysis-framework-product-scope.md);
- [package status matrix](../product/PACKAGE_STATUS.toml);
- [issue #237](https://github.com/UnknownAlienHuman/eliot-search/issues/237) including all audit-amendment comments;
- [issue #253](https://github.com/UnknownAlienHuman/eliot-search/issues/253).

## Wave-1 work

### Manager-owned implementation: #237

The manager owns one worktree and the canonical/digest foundation:

```text
crates/search-contracts/**
narrow canonical-digest guard modules and command glue in xtask
root exact crypto dependency pins where required
Cargo.lock integration
focused #237 documentation and fixtures
```

The branch stops after:

1. preservation and exact naming of the existing canonical byte profile;
2. a checked bounded canonical sink shared by vector and streaming/hash paths;
3. real BLAKE3/SHA-256 helpers with validated domain/profile/limits;
4. an honest decode/restore-versus-compute boundary;
5. digest-type taxonomy corrections accepted by #237;
6. one typed current-tree guard and shrinking allowlist;
7. current executable migration owners for every retained exception.

It does not migrate query, source, Qdrant, retention, evaluation, daemon or worker semantics.

### Manager-owned research: #253

The same manager coordinates the Unicode full-case-fold decision through a research subagent and publishes a separate documentation/research PR or artifact. It edits no `search-lexical` Rust source, manifest or `Cargo.lock`.

The output must freeze exact Unicode data/version, default/Turkic behavior, normalization order, emitted multi-scalar folds, source-range mapping, generator/dependency plan and byte-for-byte goldens so #254 makes no new donor/profile choice.

## Subagent topology

The selected packet defines eight narrow assignments:

```text
SA-01 authority/current-document map
SA-02 current-tree digest/codec inventory
SA-03 canonical codec and RFC 8949 profile review
SA-04 crypto donor/supply-chain review
SA-05 repository guard and allowlist design
SA-06 Unicode full-fold decision
SA-07 Wave-2 donor portfolio red-team
SA-08 independent final source/overengineering review
```

The manager may run any five to ten of these, but SA-01 through SA-06 and SA-08 are mandatory for #237/#253 acceptance. SA-07 may be deferred only if it reports no finding required for Wave 1.

## Donor boundary

Immediate #237 donors are selected exactly in the [donor register](./DONOR_VERIFICATION_REGISTER_2026-10-09.md):

```text
existing search-contracts codec: retain
blake3 1.8.2: exact narrow algorithm dependency
sha2 0.10.9: exact narrow algorithm dependency
ciborium 0.2.2: production codec rejected; test oracle only
```

The manager must independently recheck load-bearing source, checksum, license, MSRV, transitive features and advisories before manifest/lockfile changes. Future Wave-2 donors are selected directionally but remain `SELECTED_REVERIFY`; a detailed issue or newer version number is not sufficient qualification.

## Canonical profile safety

The current encoder orders CBOR map keys by encoded-key length and then lexicographically. This is the RFC 8949 length-first deterministic profile (§4.2.3), not Core Deterministic ordering (§4.2.1). Existing bytes and golden vectors must remain unchanged inside #237. Any ordering change requires a new canonical profile, migration and owner review.

## Wave-2 serialization

After #237 merges, coordinator #97 publishes:

1. exact post-merge `main` SHA;
2. one named `Cargo.lock` integrator;
3. a non-overlapping first batch and merge order.

Potential first foundations remain:

```text
#235 #238 #241 #246 #250 #252 #226
#256 #257 #258 #266
```

They must not start automatically. Shared `search-contracts`, root pins, `Cargo.lock`, root/control/source ownership and destructive lifecycle paths remain serialized.

## Required source gates

After code, not before:

```text
cargo +1.98.0 check --locked -p search-contracts -p xtask --all-targets
cargo +1.98.0 clippy --locked -p search-contracts -p xtask --all-targets -- -D warnings
```

Per project direction, write and compile focused golden/guard fixtures now but defer the broad/full test and installed qualification matrices. Any tiny causal test run for debugging must be reported separately and cannot become a product qualification claim.

## Stop conditions

Stop and report instead of improvising if the work would:

- change existing canonical bytes without a new profile/migration;
- introduce a second canonical codec/value tree, digest catalog, registry or journal;
- expose donor public types across ELIOT boundaries;
- accept caller-issued digest/receipt/complete/fresh authority;
- add runtime download/network fallback;
- use raw grep as the sole guard or blanket-ignore tests/archive;
- leave an occurrence unclassified or without one current executable owner;
- modify an owner package outside #237;
- start a blocked Wave-2 child merely because its issue is detailed;
- use a historical packet/donor branch as a working base;
- let a subagent write repository state or self-approve its output;
- claim Cargo, native Windows, Qdrant, installed-product or release qualification without exact evidence.

## Evidence boundary

This gate makes the project operationally ready for one manager and a bounded subagent team. It does not assert that the product is complete, every future donor is qualified, or exact-head source currently passes the required commands. The manager must produce the first exact locked check and strict-Clippy evidence for the implementation result.