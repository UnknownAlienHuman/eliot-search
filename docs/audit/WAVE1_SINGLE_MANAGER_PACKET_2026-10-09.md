# Wave 1 single-manager execution packet — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Reviewed main before this packet:** `e358d3c06d211b269efd501cd8d4002a260ba0a5`  
**Launch SHA authority:** the latest `SINGLE_MANAGER_LAUNCH_SHA` comment on [coordinator #97](https://github.com/UnknownAlienHuman/eliot-search/pull/97) after this packet is merged.  
**Topology:** one manager, one writer worktree, eight read/research/review subagents.  
**Product/release qualification:** not performed.

## Launch verdict

```text
one manager + 8 subagents: READY after this packet merge and SHA publication
broad independent writer swarm: FORBIDDEN
Wave-1 implementation scope: #237
parallel research decision: #253
Wave-2 coding: BLOCKED until #237 is merged and a new base is published
```

The manager is the only actor allowed to edit the repository, change manifests/lockfile, open implementation PRs, resolve conflicts or claim a gate result. Subagents inspect, compare, calculate and report. They do not push branches, edit the shared worktree or merge anything.

## Mandatory reading order

The manager and every subagent start here, in this order:

1. [Root `AGENTS.md`](../../AGENTS.md)
2. [Architecture entrypoint](../architecture/README.md)
3. [Normative Architecture Part I](../architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md)
4. [ADR-0005 — standalone Search/controller boundary](../adr/0005-standalone-search-product-and-controller-boundary.md)
5. [ADR-0006 — agent-analysis product scope](../adr/0006-agent-analysis-framework-product-scope.md)
6. [Package status matrix](../product/PACKAGE_STATUS.toml)
7. [Donor verification register](./DONOR_VERIFICATION_REGISTER_2026-10-09.md)
8. [Issue #237](https://github.com/UnknownAlienHuman/eliot-search/issues/237) and all audit-amendment comments
9. [Issue #253](https://github.com/UnknownAlienHuman/eliot-search/issues/253)
10. [`search-contracts/AGENTS.md`](../../crates/search-contracts/AGENTS.md), [`README.md`](../../crates/search-contracts/README.md) and [`Cargo.toml`](../../crates/search-contracts/Cargo.toml)
11. [`canonical.rs`](../../crates/search-contracts/src/canonical.rs), [`ids.rs`](../../crates/search-contracts/src/ids.rs), [`lib.rs`](../../crates/search-contracts/src/lib.rs)
12. [`conformance.rs`](../../crates/search-contracts/tests/conformance.rs) and [`publication_guards.rs`](../../crates/search-contracts/tests/publication_guards.rs)
13. [`xtask/Cargo.toml`](../../xtask/Cargo.toml), [`xtask/src/lib.rs`](../../xtask/src/lib.rs), [`xtask/src/command.rs`](../../xtask/src/command.rs)

Historical packet/tracking PRs and closed donor branches are evidence sources only. They are never working bases or merge candidates.

## Manager workspace and branch rules

The manager creates exactly one local worktree from the published `SINGLE_MANAGER_LAUNCH_SHA`.

Recommended local path and first branch:

```text
worktree: ../eliot-search-manager-wave1
branch:   manager/237-canonical-foundation
```

The manager may later create `manager/253-unicode-casefold` in the **same** worktree after committing or shelving cleanly. There is no second manager worktree. Two PRs are preferred so the #237 code merge is not blocked by #253 research:

```text
PR A: #237 code, guard, package documentation and Cargo integration
PR B: #253 donor decision and frozen golden corpus
```

Subagent reports are pasted into manager-owned notes or issue comments only after the manager verifies them. A subagent recommendation is never an accepted dependency or source change by itself.

## Scope boundary

### Manager-owned implementation — #237

Allowed repository writes:

```text
crates/search-contracts/**
xtask/src/canonical_digest_guard.rs or an equivalent narrow guard module
xtask/src/lib.rs
xtask/src/command.rs and narrow command glue
xtask/Cargo.toml only when required
root Cargo.toml exact crypto pins only when required
Cargo.lock integration
focused documentation/fixtures for #237
```

The manager does **not** migrate query, source, Qdrant, retention, evaluation, daemon or worker semantics in this branch. The current-tree occurrences become typed allowlist entries with current executable owner issues.

### Manager-owned research — #253

Allowed writes are documentation, exact Unicode data/checksum records, generator design and golden fixtures. Do not modify `search-lexical`, Rust manifests or `Cargo.lock` in the #253 PR.

### Explicitly blocked

Do not start #235, #238, #241, #246, #250, #252, #226, #256, #257, #258 or #266 coding before #237 merges and a new Wave-2 base/lock integrator is published.

## Subagent contract

Every subagent receives one small assignment below and returns exactly:

```text
assignment ID
base SHA inspected
files/specs/releases inspected
current facts with path + symbol or source line
contradictions/stale references
recommended action: ACCEPT / REJECT / BLOCK / NO_CHANGE
exact output artifact requested by the assignment
uncertainty and missing evidence
no implementation or qualification claim
```

No generic prose dump. No architecture invention. No repeating issue text without checking current source.

---

# Subagent assignments

## SA-01 — Normative authority and execution-map audit

**Mission:** produce the one-page authority map the manager uses to reject stale instructions.

**Must read:** mandatory reading items 1–9, plus [master audit](./ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md), [launch gate](./AGENT_LAUNCH_GATE_2026-10-09.md) and [coordinator #97](https://github.com/UnknownAlienHuman/eliot-search/pull/97).

**Check:**

- current versus historical SHA references;
- `search-contracts` versus obsolete `search-canonical` wording;
- one-manager versus old two-manager wording;
- current executable issues versus old packet/donor issue numbers;
- owner/file boundaries and dependency order;
- package instructions that contradict #237.

**Output:** `AUTHORITY_MAP` with precedence, current launch entrypoint, exact stale references and the document/issue that supersedes each. Do not edit files.

**Stop:** report `BLOCK` if two current documents grant overlapping write authority or prescribe different canonical byte profiles.

## SA-02 — Current-tree canonical/digest inventory

**Mission:** replace the stale path list in #237 with an exact current-tree inventory.

**Start at:**

- [`ids.rs`](../../crates/search-contracts/src/ids.rs)
- [`canonical.rs`](../../crates/search-contracts/src/canonical.rs)
- every current occurrence of `Blake3Digest32::from_bytes`, `Sha256Digest32::from_bytes`, `VersionedContentDigest`, custom `digest32`, SHA constants/compression loops, digest-prefix operation IDs and package-local canonical writers.

**Classify every occurrence:**

```text
production compute
wire/store decode
real accepted algorithm implementation
semantic fingerprint/generation
honest noncrypto fingerprint
test/vector fixture
optional harness
archive/history
unknown — manager must inspect
```

**Output:** a machine-friendly table:

```text
path | symbol | type produced | actual algorithm/preimage | classification |
current executable owner issue | allowlist action | evidence
```

Use current package paths. Do not copy old names such as `crates/search-access`, `search-collection-manager` or obsolete donor package layouts unless they actually exist on the launch SHA.

**Stop:** do not propose mass migrations. #237 owns the shared API and guard only.

## SA-03 — Canonical codec and RFC 8949 profile review

**Mission:** verify the existing encoder/decoder and freeze exact bytes before crypto integration.

**Primary sources:**

- [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html), especially §§4.1, 4.2.1 and 4.2.3
- [`canonical.rs`](../../crates/search-contracts/src/canonical.rs)
- [conformance fixtures](../../crates/search-contracts/tests/conformance.rs)
- [Ciborium 0.2.2 canonical source](https://github.com/enarx/ciborium/blob/v0.2.2/ciborium/src/value/canonical.rs)

**Check:**

- definite lengths, shortest integer/length encodings and duplicate rejection;
- current length-first map-key ordering versus Core Deterministic ordering;
- strict decode/re-encode behavior;
- JSON byte wrapper and reserved-key behavior;
- allocation before output ceiling;
- double materialization in `domain_separated_preimage`;
- whether vector and streaming/hash paths can share one encoder without changing bytes.

**Output:** `CANONICAL_PROFILE_FREEZE` containing exact profile name, current golden vectors that must remain byte-identical, divergences from RFC profiles, bounded-sink requirements and `NO_CHANGE`/migration decision for each finding.

**Critical rule:** never change map ordering under the same profile. A standards-label correction is documentation; a byte change is a new profile/generation.

## SA-04 — Crypto donor and supply-chain review

**Mission:** independently verify the exact #237 dependency profile.

**Primary sources:**

- [`blake3` 1.8.2 Cargo metadata](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.2/Cargo.toml)
- [`sha2` 0.10.9 Cargo metadata](https://github.com/RustCrypto/hashes/blob/sha2-v0.10.9/sha2/Cargo.toml)
- [BLAKE3 official repository/test vectors](https://github.com/BLAKE3-team/BLAKE3)
- [RustCrypto hashes](https://github.com/RustCrypto/hashes)
- [`Cargo.lock`](../../Cargo.lock)
- [donor register](./DONOR_VERIFICATION_REGISTER_2026-10-09.md)

**Check:** exact tags/commits/checksums/licenses/MSRV, normal/build dependency closure, features, advisories, existing lockfile reuse and Rust 1.98 compatibility.

Specifically verify the risk that `blake3::pure` is upstream-documented as unstable/testing-only. Compare:

```text
exact pin + std + pure + zeroize
exact pin + std + zeroize with optimized/native implementations
```

Recommend one profile based on attack surface, build reproducibility, Windows/MSVC behavior and output parity. Do not choose on throughput alone.

**Output:** one `DEPENDENCY_ACCEPTANCE_RECORD` per crate and a final `ACCEPT` or `BLOCK` for manifest modification.

## SA-05 — Guard architecture and allowlist review

**Mission:** design a useful Rust-aware repository guard rather than a grep wall that agents disable.

**Read:** #237 and all comments, [`xtask/src/lib.rs`](../../xtask/src/lib.rs), [`xtask/src/command.rs`](../../xtask/src/command.rs), existing `qdrant_boundary` validation patterns and SA-02 inventory.

**Required detection classes:**

- custom/FNV-like bytes entering algorithm-qualified digest types;
- copied SHA constants/padding/compression outside exact fixtures;
- truncated digest prefixes used as mutation/replay/evidence authority;
- duplicate generic canonical writers/fingerprint builders;
- ambiguous raw-byte construction outside decode/restore/accepted compute locations;
- new direct crypto/codec dependency without owner/profile record.

**Required exclusions:** real hash use, wire/store decode, fixtures, optional harness, archive/history and honestly named noncrypto fingerprints.

**Output:**

1. exact guard module/command entrypoint;
2. typed allowlist schema (`path`, `symbol/pattern`, `classification`, `reason`, `owner`, `removal phase`);
3. self-fixture matrix covering false positives and false negatives;
4. list of what syntax/source structure can and cannot prove;
5. stop rule for `COMPILER_REQUIRED` or manual review.

No raw-substring-only implementation recommendation is accepted.

## SA-06 — Unicode full-fold decision (#253)

**Mission:** freeze an implementable `prose_words@1` case-fold input for #254 without coding it.

**Primary sources:**

- [Unicode 18.0.0 `CaseFolding.txt`](https://www.unicode.org/Public/18.0.0/ucd/CaseFolding.txt)
- [UAX #44](https://www.unicode.org/reports/tr44/)
- [UAX #15](https://www.unicode.org/reports/tr15/)
- [UAX #31](https://www.unicode.org/reports/tr31/)
- [issue #253](https://github.com/UnknownAlienHuman/eliot-search/issues/253)
- exact candidate crate source, not only docs.rs summaries.

**Compare:**

1. exact checked-in generated table from Unicode 18.0.0;
2. a small maintained crate with emitted full/default/Turkic folding and explicit Unicode version;
3. ICU4X only if the first two cannot satisfy exact output/range requirements.

Comparison-only APIs are insufficient unless they expose the exact transformed sequence needed for indexing and source-range mapping.

**Output:** `CASEFOLD_DECISION` containing:

```text
selected source/release/data checksum/license/MSRV
Unicode version
C+F and optional T policy
NFC/NFKC and exact order around folding
multi-scalar output API
source-range mapping rule
generator/update procedure
binary/table/dependency cost
golden input -> scalar sequence -> UTF-8 bytes
rejected alternatives with concrete reason
instructions for #254 requiring no further choice
```

Minimum golden corpus: ASCII, `ß/ẞ`, `Σ/σ/ς`, `I/İ/ı/i`, composed/decomposed combining cases, multi-scalar folds, supplementary-plane folds and unchanged characters.

## SA-07 — Wave-2 donor portfolio red-team

**Mission:** verify that future issues use mature mechanisms and are not quietly importing a second framework/authority.

**Entry issues and donors:**

- [#238 TOML](https://github.com/UnknownAlienHuman/eliot-search/issues/238)
- [#241 globset](https://github.com/UnknownAlienHuman/eliot-search/issues/241)
- [#246–#248 exact matchers](https://github.com/UnknownAlienHuman/eliot-search/issues/246)
- [#250–#251 cargo_metadata/syn](https://github.com/UnknownAlienHuman/eliot-search/issues/250)
- [#252–#254 Unicode profiles](https://github.com/UnknownAlienHuman/eliot-search/issues/252)
- [#226 Markdown/JATS](https://github.com/UnknownAlienHuman/eliot-search/issues/226)
- [#129 gitoxide](https://github.com/UnknownAlienHuman/eliot-search/issues/129)
- [#223/#224 Tree-sitter/SCIP](https://github.com/UnknownAlienHuman/eliot-search/issues/223)
- [donor register](./DONOR_VERIFICATION_REGISTER_2026-10-09.md)

**For each family check:** donor exactness, capability fit, hidden defaults, resource/cancellation limitations, public-type leakage, network/runtime downloads, duplicate authority and smaller alternatives.

**Output:** a compact table:

```text
slice | selected donor | retain | reject | missing decision/evidence |
ACCEPT / AMEND_ISSUE / REPLACE / BLOCK
```

This subagent does not implement Wave 2 and does not churn versions merely because a newer release exists.

## SA-08 — Independent integration and overengineering review

**Mission:** act as adversarial reviewer after the manager has a candidate #237 diff and #253 decision.

**Inputs:** candidate diff, all seven reports, issue acceptance boundaries, package/root instructions and exact check/Clippy output.

**Review questions:**

- Did the manager create a second codec, digest catalog or generic receipt type?
- Did any old canonical byte change without a new profile?
- Is the bounded sink actually bounded before growth/write?
- Is decode/restore separated honestly from compute?
- Are semantic fingerprints still being blessed as generic algorithm digests?
- Can `VersionedContentDigest` or equivalent public fields still relabel arbitrary bytes?
- Does the guard classify rather than blanket-ignore tests/archive?
- Did dependency features widen beyond the accepted record?
- Did #253 leave any choice to #254?
- Are old paths/issues/docs still presented as current?
- Did the diff touch owner packages outside scope or add compatibility code that remains product-reachable?

**Output:** `FINAL_REVIEW` with blocking findings only, each tied to a file/symbol/invariant. `APPROVE_SOURCE` means the source boundary is coherent; it is not product qualification.

---

# Manager execution sequence

## Phase 0 — freeze authority and inventory

Start SA-01 through SA-07 concurrently. The manager reads current code directly and begins no manifest change until SA-01, SA-03 and SA-04 return without `BLOCK`.

SA-02 produces the current allowlist seed. The issue body’s older path table is not treated as exhaustive or authoritative.

## Phase 1 — code first: bounded canonical core

The manager implements the smallest coherent #237 source slice:

1. freeze/document the existing length-first canonical profile and current bytes;
2. introduce one private checked bounded sink shared by vector and streaming/hash output;
3. remove output growth past the accepted ceiling before rejection;
4. preserve one encoder implementation for returned CBOR and bytes fed to hashers;
5. introduce validated canonical digest domains and limits;
6. add real BLAKE3/SHA-256 compute helpers behind `search-contracts`;
7. separate decode/restore of existing bytes from computation without pretending the migration is already complete;
8. make algorithm-tagged content digest fields private/validated where included in #237’s accepted scope;
9. avoid a generic operation receipt that callers can mint from arbitrary values.

Do not wait for a broad test campaign before writing code. Do not migrate every consumer.

## Phase 2 — guard and migration ledger

Using SA-02 and SA-05:

1. add the typed current-tree allowlist;
2. add a bounded guard command under `xtask`;
3. add guard self-fixtures/source samples;
4. assign every retained exception to one current executable owner issue;
5. record legacy profile/rebuild requirements rather than relabelling bytes.

## Phase 3 — Unicode decision

The manager verifies SA-06 against primary source and writes the #253 decision artifact. If no candidate provides emitted full-fold sequences with the required provenance/range behavior, select the exact generated-table fallback instead of adopting ICU4X by convenience.

## Phase 4 — independent review and minimal gates

Run SA-08 over the final candidate.

Required now, after code:

```text
cargo +1.98.0 check --locked -p search-contracts -p xtask --all-targets
cargo +1.98.0 clippy --locked -p search-contracts -p xtask --all-targets -- -D warnings
```

`--all-targets` must compile the focused fixture targets. Per project direction, do not run the broad/full test matrix in this implementation wave. Write the focused golden/guard fixtures now and retain their exact names; execute the complete focused/product/native matrices in the later testing/qualification phase. If the manager runs any tiny causal test to debug a local defect, report it separately and do not convert it into a qualification claim.

No GitHub status or workflow result exists automatically for the launch base. The manager must paste exact local command output and environment/toolchain details into the PR.

## Phase 5 — handoff

The manager reports:

```text
launch/base SHA and result SHA
one worktree and branch history
subagents launched and report disposition
exact files/packages changed
old canonical profile and golden bytes preserved/changed
exact dependencies/checksums/features/licenses/advisories
bounded sink/domain/API ceilings
complete typed allowlist and current owner issue
custom code deleted or avoided
locked check outcome
strict Clippy outcome
focused fixtures written but execution status stated honestly
#253 selected profile/data/goldens
blocking findings and deferred owner migrations
no claim above IMPLEMENTED/CHECKED
```

After #237 merges, coordinator #97 publishes the exact post-merge SHA, one lockfile integration order and the first non-overlapping Wave-2 batch. Subagents do not continue into blocked issues automatically.

## Stop conditions

Stop and report instead of improvising when any of the following occurs:

- canonical bytes differ without an explicitly accepted new profile/migration;
- package/root instructions still conflict after reading this packet and current issue comments;
- exact donor source/API contradicts the selected profile;
- `blake3::pure` or another feature fails the accepted Rust/target build and no reviewed alternative exists;
- a second codec, canonical value tree, digest catalog, client, registry or journal would be introduced;
- a donor public type would cross an ELIOT contract/protocol boundary;
- a guard requires blanket ignore of tests/archive or would be raw grep only;
- a current occurrence cannot be classified or assigned to one executable owner;
- the change requires editing an owner package outside #237;
- #253 cannot freeze exact data/version/normalization/Turkic outputs;
- a subagent attempts repository writes or represents its report as acceptance;
- a broad test/qualification/release claim is requested from this source-only wave.

## Readiness statement

This packet makes the work **operationally ready for one manager with eight subagents**. It does not say the product is ready, every donor is fully qualified, or current source passes exact-head compilation. The immediate donors and code entrypoints are sufficiently specified to start #237 safely; unresolved Unicode and future donor choices remain explicit gates rather than hidden assumptions.