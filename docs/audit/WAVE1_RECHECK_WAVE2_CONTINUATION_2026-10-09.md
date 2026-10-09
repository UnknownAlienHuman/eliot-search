# Wave 1 recheck and Wave 2 continuation — 2026-10-09

**Reviewed implementation snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Verdict:** Wave 1 has been delivered; Wave 2 has already completed its first two implementation slices. Continue with #256, not with another foundation rewrite. Product/native/Qdrant/release qualification remains open.

## 1. Verified delivery versus claimed execution

| Task | Merged PR | Reviewed source head | Result |
|---|---|---|---|
| #317 | [#318](https://github.com/UnknownAlienHuman/eliot-search/pull/318) | `7a3f07f1c27d21cd4bbb2af9ac083385c89cae83` | Existing xtask compilation/lint repair |
| #237 | [#319](https://github.com/UnknownAlienHuman/eliot-search/pull/319) | `99918613044952be7a0b1a919aa99bf5ba0e8cfb` | Canonical streaming/bounds, real hashing and source guard |
| #253 | [#322](https://github.com/UnknownAlienHuman/eliot-search/pull/322) | `0968175765579376bd05a52828245f9ff8ba2bb3` | Unicode full-fold decision/data/goldens, not a production tokenizer |
| #258 | [#323](https://github.com/UnknownAlienHuman/eliot-search/pull/323) | `26b3a0ef54c41b022ca88285eee2e0b0c0fe9a0f` | Shared indexed schema/eligibility contract |
| #250 | [#326](https://github.com/UnknownAlienHuman/eliot-search/pull/326) | `5c796c91a32d67a4113f83c14ad625c0ec314165` | Bounded Cargo metadata and status/dependency validation |

The #326 source head and merged implementation snapshot have no file differences; the recorded tree is `cc60701e25a4e3bd60903a558b14e7a5c3ac1692`.

Recorded author evidence:

- [#319 final-head record](https://github.com/UnknownAlienHuman/eliot-search/pull/319#issuecomment-6086319977): Windows Rust 1.98 scoped locked all-target/all-feature check and strict Clippy exit 0; canonical source guard exit 0 over 4,611 sites. Revision-store consumer Clippy remains a separately reported pre-existing failure.
- [#326 final-head record](https://github.com/UnknownAlienHuman/eliot-search/pull/326#issuecomment-6088136519): scoped locked/offline check and strict Clippy, 176 focused fixtures, source guard over 4,658 sites. The full real workspace validator returns exit 1 for exactly the stale evaluation README (#324).
- [Coordinator readback](https://github.com/UnknownAlienHuman/eliot-search/pull/97#issuecomment-6088153699): advances to #256 and records the exact source/merge tree and remaining follow-ups.

This audit verified repository state, inspected source and read those records. Rust/Cargo are unavailable in the audit runtime; it did not independently rerun the reported Windows commands. Manual-only Actions are the repository's policy, so absence of automatic CI is not by itself an implementation failure. A textual PASS record still has a narrower provenance than retained raw logs or an independent rerun.

## 2. What the source review confirms

### Canonical foundation

[Current digest source](https://github.com/UnknownAlienHuman/eliot-search/blob/17383079d316cc090eae04912534e2d16e6dc79d/crates/search-contracts/src/digest.rs) uses the real private BLAKE3 and RustCrypto SHA-256 implementations. Domain parsing distinguishes CBOR and raw payloads; input limits include the domain and separator. Partial encoding failure returns no digest.

The canonical encoder patch checks the sink before writes, streams base64 rather than constructing a complete temporary string, feeds hashing from the same CBOR encoder and removes the second full preimage buffer. For the closed text-key vocabulary, length then byte sorting preserves the existing encoded-key order. This is [RFC 8949 section 4.2.3](https://www.rfc-editor.org/rfc/rfc8949.html#section-4.2.3), not a silent switch to another deterministic profile.

These are substantive fixes. They do not migrate every package-local legacy identity or prove the semantic meaning of every digest in the source ledger.

### Shared indexed contract

[Contract documentation](../../crates/search-contracts/INDEXED_CONTRACT.md) and [eligibility implementation](../../crates/search-contracts/src/indexed/eligibility.rs) retain one closed payload/index plan, a generation-bound epoch, private membership storage and a single population used by retrieval/IDF/count/scroll consumers. The package grants no access or source authority by itself. The actual bridge and scope owners still have to use it.

### Cargo metadata

[The adapter](../../xtask/src/cargo_metadata_adapter.rs) constructs the official Cargo command, captures stdout/stderr separately with byte caps, uses one deadline and refuses unsuccessful/invalid output. It does not use the donor's unbounded convenience `exec()` as a fake bounded runner. Remaining lifecycle limitations are recorded below rather than advertised as complete process-tree containment.

## 3. Corrections to our previous implementation plan

### W2-R1 — #256 deletion order could break consumers

**Evidence:** [daemon composition](../../bins/eliot-searchd/src/projection_composition/kernel/compose.rs) imports and constructs `PointIdentityRegistry`; [its ownership fixture](../../bins/eliot-searchd/tests/projection_composition_module_ownership.rs) also expects that symbol. The old #256 issue simultaneously required deleting the export and forbade consumer edits.

A green `-p search-point-identity` check would not prove those consumers still compile. This was a planning contradiction, not an agent's fault.

**Correction:** #256 introduces a separate stateless S11 profile using the current shared contracts. Existing legacy exports remain unchanged only until their existing consumers cut over under #259/#262 and daemon integration. The new profile cannot fall back to legacy or admit legacy IDs into a new generation. The removal ledger and reverse-consumer checks are mandatory. Final product qualification cannot pass while old production routing silently remains.

No new generic compatibility framework, copied codec or second identity authority is permitted. This is explicit expand/contract migration of existing source, not indefinite duplication.

### W2-R2 — domain-bound SHA is not a byte-compatible old checksum

[Old config code](../../crates/search-config/src/fingerprint.rs) computes `SHA256(legacy_preimage)`; that preimage already includes the old config framing. The merged shared `sha256_raw` computes `SHA256(domain || NUL || bytes)` and requires a nonempty validated domain.

Therefore this substitution does not preserve v1 bytes:

```text
old: SHA256(legacy_preimage)
new helper on old bytes: SHA256(new_domain || NUL || legacy_preimage)
```

**Correction:** #238 gets a narrowly scoped shared exact-byte SHA-256 primitive for compatibility verification, using the existing private RustCrypto donor and an explicit input ceiling. Existing domain-bound APIs and their goldens remain unchanged. Official standard vectors and exact old-preimage parity precede removal of copied SHA. New config v2 identities still use their explicit canonical domain and migration policy. #241 reuses the accepted helper afterward.

A checksum is not a receipt or authorization proof. No empty-domain workaround, fake stored constructor, unbounded reader or second hashing crate is permitted.

### W2-R3 — documentation still said completed work had not started

The launch gate and audit entrypoint still blocked all Wave 2 on #237 and directed agents to restart Wave 1. This review replaces that live status with the merged facts and remaining queue. Historical master-audit diagnoses remain available, but are not live execution state.

#324 fixes the separate active evaluation README that still described existing source as absent. The validator predicate, source code and status matrix are not weakened. The manager must rerun the real current-workspace command after the documentation repair; this audit does not invent that command's result.

## 4. Remaining source-level follow-ups, not reasons to restart Wave 1

### Membership validation complexity

`EligibilityPopulation::new` checks duplicates by searching every prefix before copying and sorting. With the declared maximum 4,096 members, all-distinct input performs up to 8,386,560 membership comparisons. This is bounded, but unnecessarily quadratic.

A narrow future improvement is: reject empty/oversize input, copy into one capped vector, sort once, reject equal adjacent entries, then construct the private set. Preserve all error classes and canonical bytes. The getter also returns a copied Vec; an additive borrowed iterator would avoid repeated copies for internal consumers. No latency measurement was performed, so this is an algorithmic observation, not a benchmark claim or a release-blocking incident.

### Metadata child/reader cleanup

`run_bounded` starts two reader threads without retaining their join handles. `stop_child` polls once and calls `kill`, but does not establish completed reaping. A descendant retaining a pipe can leave readers blocked even after the direct child exits. The wrapper bounds the caller's wait/output, not every descendant/thread lifetime.

Keep this distinction explicit in tooling acceptance. A focused follow-up should define and test the supported process/pipe cleanup contract, including timeout, spawn failure and inherited-handle cases. Do not import a production process supervisor or async runtime just to hide the limitation. This audit did not reproduce an OS-level leak.

### Existing Rustls advisory

[#327](https://github.com/UnknownAlienHuman/eliot-search/issues/327) tracks Rustls 0.23.44. The [upstream advisory](https://github.com/rustls/rustls/security/advisories/GHSA-2mjx-qc3c-rqvc) identifies affected versions 0.23.13–0.23.44 and fix 0.23.45; [RustSec](https://rustsec.org/advisories/RUSTSEC-2026-0285.html) confirms the patched floor.

Do not inflate this into an authentication-bypass claim: the advisory states the handshake transcript remains authenticated. Patch the affected closure through the one manager before accepting TLS/network use. The new Cargo metadata donor closure did not introduce that dependency, and pure S11 work need not wait for an unrelated network operation.

### Legacy bounds provenance and delegation policy

[#325](https://github.com/UnknownAlienHuman/eliot-search/issues/325) keeps the P00 table-digest provenance unresolved under its own owner. Closing #258 did not silently bless the constant.

The implementation handoff references a maintainer allowance for code-writing subagents, while root instructions still say read-only. Retain the exact instruction and reconcile a narrow ownership rule in #97/root before delegating writes. Do not infer misconduct from an incomplete GitHub account of local work, and do not use the handoff alone as authorization. One manager remains the integrator.

## 5. Current continuation

```text
DELIVERED: #317, #237, #253, #258, #250
NEXT:      #256
THEN:      #257 → #266 → #235 → #238 → #241 → #246 → #252
DOCUMENTS: #226 Markdown → #226 JATS
```

#324 is a documentation repair. #327 is the narrow security-maintenance lane. Neither authorizes more independent managers or an expansion of the active product slice.

Each active source PR must record its exact base/head, changed public APIs, direct consumers, existing versus newly introduced failures, accepted donor closure and scoped locked check/strict Clippy. Reuse completed donor research and source; do not regenerate a broad audit or a new task hierarchy at every step.

## 6. Readiness boundary

**Ready:** controlled continuation from the accepted post-#250 base, using the corrected #256 staged port and one manager.

**Not established:** full Wave-2 completion; every legacy digest migrated; current whole-workspace strict Clippy; process-tree cleanup under every platform failure; live Qdrant, installed Windows, scale/disclosure/release qualification.

Only instruction/documentation changes are made by this review. No implementation result, raw execution log or security qualification is fabricated.
