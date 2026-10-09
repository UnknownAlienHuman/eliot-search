# Wave 2 single-manager execution packet — 2026-10-09

**Verified source snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97).  
**State:** Wave 1 delivered; Wave 2 already in progress. Next implementation: **#256**.  
**Integration topology:** one manager, one integration worktree, 5–10 narrowly scoped subagents. The manager alone controls commits, merges, dependency pins and Cargo.lock.

This revision replaces the obsolete instruction to wait for #237 or begin again with #258. Do not repeat delivered work or reset an active worktree to an old launch SHA.

## 1. What is already delivered

| Task | Merged PR | Actual delivery |
|---|---|---|
| #317 | #318 | Existing xtask source-gate repairs |
| #237 | #319 | Bounded canonical encoder, real digest helpers, stored/compute distinction, source-classification guard |
| #253 | #322 | Unicode 18 full-fold decision and 39 frozen golden cases; not a production tokenizer |
| #258 | #323 | `search_contracts::indexed`: one payload/schema/eligibility/epoch contract |
| #250 | #326 | Bounded Cargo metadata adapter and package-status/dependency validation |

The final #326 reviewed head is `5c796c91a32d67a4113f83c14ad625c0ec314165`; the merged source snapshot above has the same file tree. Its author records locked/offline Windows Rust 1.98 all-target/all-feature check and strict Clippy plus 176 focused fixtures. These are recorded source-check results, not an independent runtime rerun by this audit or product qualification.

The real workspace validator intentionally returned exit 1 for the stale search-eval README (#324). A documentation repair must remove that violation, never weaken the predicate or hide the error.

## 2. Read only what the active slice needs

1. [Root instructions](../../AGENTS.md) and nearest package instructions.
2. [Current launch gate](AGENT_LAUNCH_GATE_2026-10-09.md) and latest accepted-base comment on #97.
3. This packet and the [exact package exceptions](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md).
4. The assigned issue, its named normative section and current source.
5. The accepted donor/source profile for that issue, not an obsolete candidate pin.

Useful entrypoints:

- [Merged canonical foundation and API semantics](CANONICAL_FOUNDATION_IMPLEMENTATION_2026-10-09.md).
- [Actual digest API](../../crates/search-contracts/src/digest.rs).
- [Indexed contract](../../crates/search-contracts/INDEXED_CONTRACT.md).
- [Indexed source modules](../../crates/search-contracts/src/indexed/).
- [Indexed donor port map](INDEXED_SPINE_PORT_MAP_2026-10-09.md).
- [Wave-2 donor register](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md).
- [Cargo metadata delivery](WAVE2_CARGO_METADATA_CUTOVER_2026-10-09.md).
- [Central package status](../product/PACKAGE_STATUS.toml).

Older audit snapshots explain defects. They do not override delivered code or authorize a second implementation.

## 3. Remaining serialized work

```text
#256 S11 identity foundation with build-safe legacy retention
→ #257 VerifiedUnitSet v3
→ #266 typed root open/inspect/init/recovery
→ #235 canonical typed provider client
→ #238 typed TOML + explicit legacy SHA compatibility
→ #241 compiled relative-path admission
→ #246 literal matchers
→ #252 XID + NFC code identifiers
→ #226 Markdown
→ #226 JATS
```

#327 is a separate minimal Rustls security update through the same lockfile integrator. Do it before accepting any affected TLS/network profile; it does not prevent read-only review or pure point-identity work. #325 remains one legacy bounds-provenance follow-up, not permission to reopen all completed shared contracts.

Do not restart #237/#253/#258/#250. Do not start #259–#264, full code-analysis profiles, destructive lifecycle or release merely because an API compiles.

## 4. Next task: #256

Source entrypoints:

```text
crates/search-index-qdrant/search-point-identity/src/lib.rs
crates/search-contracts/src/digest.rs
crates/search-contracts/src/indexed/{mod,fields,payload,collection,eligibility}.rs
bins/eliot-searchd/src/projection_composition/kernel/compose.rs   # read-only consumer inventory
bins/eliot-searchd/tests/projection_composition_module_ownership.rs
```

Source donor: [#207 immutable snapshot](https://github.com/UnknownAlienHuman/eliot-search/tree/f0ac8a1470336475f28e3fb30610947c83bbb6d1). Port only the named key/collision/UUID model and fixtures. Do not merge the branch or copy its canonical writer or direct crypto owner.

Implement the accepted eight-field S11 key, full BLAKE3 identity, separately domain-bound 128-bit address, reversible UUID representation and stateless collision decision through the merged canonical API. Freeze the new domains/profile/goldens explicitly; never reinterpret old identities.

**Build-safe migration:** current daemon code imports `PointIdentityRegistry`. A package-only PR cannot delete that export and simultaneously forbid consumer edits. Add the new S11 profile first; retain only the unchanged existing legacy surface for current consumers, with an exact removal ledger. No S11-to-legacy fallback, new wrapper framework or acceptance of legacy identity in the new generation. #259/#262 and daemon cutover remove those consumers and exports. This is the narrow exception in the matrix, not permission for indefinite dual authority.

Completion of #256 means the new profile is ready for downstream ports and consumer build compatibility is preserved. It does not mean the indexed product or legacy deletion program is complete.

## 5. Eight reusable subagent assignments

| Agent | Read scope | Return artifact |
|---|---|---|
| SA-01 | Active issue, package instructions, exact callers | `AUTHORITY_AND_CALLER_MAP`: prerequisites, permitted paths, reverse consumers, contradictions |
| SA-02 | Existing lock, admitted donor reports and primary source | `DEPENDENCY_DELTA`: exact version/checksum/features/advisories; reuse accepted pins |
| SA-03 | Current indexed contract and immutable #207 donor | `S11_PORT_MAP`: fields/domains/vectors to retain, codec/state to reject |
| SA-04 | Unitizer/materializer interfaces for #257/#226 | `UNITSET_MAP`: actual APIs, coordinate basis, complete/partial semantics |
| SA-05 | Root/client callers for #266/#235 | `COMPOSITION_MAP`: all opens, typed client versus legacy shim, deletion owners |
| SA-06 | Config/admission for #238/#241 | `MIGRATION_MAP`: exact old/new preimages, parser options, budgets and disclosure |
| SA-07 | Active candidate patch | `REGRESSION_MAP`: canonical parity, complexity, downstream imports, cleanup paths |
| SA-08 | Exact final candidate SHA and manager command records | Blocking findings or scoped `APPROVE_SOURCE`; not a product PASS |

Launch the relevant assignments, not eight full-repository audits on every slice. Avoid generating another general task registry. Reviews are pinned to the candidate SHA and expire when relevant code changes.

The checked-in root policy currently makes subagents read/research/review-only. A delivery report references a later maintainer allowance for code-writing subagents. That report alone does not amend root policy: the manager must retain the exact maintainer instruction and reconcile the narrow file-ownership rule before assigning writes. Do not create a second integration manager or let subagents mutate root pins/lockfile/PR state.

## 6. Donor integration corrections

### Shared SHA is not an interchangeable legacy checksum

`sha256_raw` computes `SHA256(domain || NUL || bytes)`. Existing config code computes `SHA256(legacy_preimage)` where the legacy framing is already inside the bytes. Calling the former on the latter adds another prefix and changes the result.

#238 must use the narrowly reviewed exact-byte SHA extension described in the package exception matrix for v1 parity; new v2 identities use the domain-bound canonical helper. Do not change the existing shared API's semantics or silently relabel fingerprints. #241 can reuse the accepted extension after #238.

### Current donor records supersede preparation guesses

#237 already adopted the shared digest/AST tooling closure; #250 already adopted Cargo metadata. Do not independently upgrade or re-add those libraries in #256. #253 selected no normalization in its full-fold-only profile; that does not remove NFC from #252's distinct code profile.

For later parsers, verify published archive features/checksum before lock changes. `quick-xml` positions require source mapping verification, not a claim of automatic semantic ranges. Markdown/JATS consume #257's complete UnitSet, not a provisional document catalog.

## 7. Manager procedure and source gates

Use the latest accepted #97 SHA. Preserve the existing manager worktree; start a clean issue branch in it, never reset another actor's work. One candidate PR at a time.

```text
read active issue/current APIs
→ targeted subagent reports
→ code in the owning package
→ check direct reverse consumers of changed public APIs
→ locked check and strict Clippy
→ exact candidate review
→ PR/merge
→ publish next accepted SHA
```

For #256:

```text
cargo +1.98.0 check --locked -p search-contracts -p search-point-identity --all-targets --all-features
cargo +1.98.0 clippy --locked -p search-contracts -p search-point-identity --all-targets --all-features -- -D warnings
cargo +1.98.0 run --locked -p xtask -- validate canonical-digest-guard
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary
```

Compare direct-consumer compilation before and after a public API change. A pre-existing failure must be recorded separately; no new unresolved import/type error may be hidden behind a package-only PASS. Run only necessary focused proof, not a broad product/native test cycle. Actions remain manual-only.

A ledger refresh may update exact changed sites; it cannot reclassify a pending fake/legacy digest as accepted merely because an issue closed. Keep filenames, source hashes, classification, phase and executable owner truthful.

## 8. Stop conditions

Stop the affected slice, not the entire project, when an API needed for its next step is missing, a new source/profile would reinterpret legacy bytes, or package-only changes break a forbidden consumer. Amend scope/sequence rather than inventing a replacement owner or disabling the checker.

Never claim product/native/Qdrant readiness from a merged source PR, a signature, a security-review badge or a source-ledger PASS. This packet authorizes the remaining controlled implementation sequence only.
