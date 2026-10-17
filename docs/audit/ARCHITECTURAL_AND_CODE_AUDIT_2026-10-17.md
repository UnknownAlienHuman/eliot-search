# ELIOT Search — Gemini audit verification and refactoring preparation

**Verification date:** 2026-10-17. **Source baseline:** c8e38b6fc9829f1cd38070773b1b3e5b21de18ad,
the Gemini documentation commit over 239c572348dc6491f97de65464e889d41e25da80 (PR #609).
[Original Gemini report](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).

This revision verifies and corrects that report; it does not assert that its proposed repairs have landed.
Architecture 8.4 Part I and accepted ADRs define product semantics. This document is source evidence and
a bounded work map, not a second architecture, a launch prerequisite or the live writer queue.
Use [EXECUTION_PROTOCOL](../product/EXECUTION_PROTOCOL.md) for implementation and
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) for the current manager handoff.

## 1. Result and review scope

The main problem is incomplete composition of existing owners into a working indexed product.
The repository has real source-admission, canonical hashing, S11 identity, planner and publication
admission implementations. It also retains obsolete implementations, noncryptographic semantic
fingerprints and disconnected daemon compositions. Reimplementing the delivered owners would add
another parallel implementation.

Reviewed: Part I, all ADR files (including the two differently scoped ADR 0003 files), root/package
instructions and execution maps; Cargo targets/features; daemon dispatch and relevant source-reader
paths; canonical/S11/planner/publication/bridge boundaries; the audit's digest sites; and current
#259/#260/#262/#329/#352/#598 context. This is a targeted static review, not a line-by-line review of all
Rust files. No compile, Clippy, test, Windows or live-Qdrant execution is claimed by this preparation.

### Measured size at the source baseline

| Measure | Exact result | Interpretation |
|---|---:|---|
| Cargo workspace members | 52 | 47 library-area packages, four binary-area packages, xtask |
| Tracked Rust files / physical lines | 1,593 / 393,383 | Includes comments, blank lines, tests and harnesses |
| Daemon Rust files / physical lines | 604 / 122,212 | Not a production-only LOC measurement |
| Digest ledger | 3,572,618 bytes / 72,002 lines | 5,960 classified sites, not 72,002 defects |
| Ledger phases | 4,708 accepted; 1,252 migration/removal | Includes legitimate algorithms, opaque uses, codecs and test vectors |
| swarm/coverage | 205 files / 5,146,354 bytes / 152,921 lines | Historical development metadata |
| GitHub before this preparation PR | 305 total PRs; 121 open issues; no open PRs | #609 is a shared issue/PR number, not a PR count |

File counts use tracked files only; physical lines count newline bytes. Cargo members come from
Cargo.toml. GitHub counts use repository-scoped issue search with is:pr and is:issue is:open;
the search responses reported incomplete_results=false. Counts are a dated inventory, not performance
evidence. Gemini's approximately 82.6k daemon LOC used an unspecified counting method and is not
directly comparable to the physical-line count above.

## 2. Verified findings and corrections

Paths and line anchors below refer to the source baseline, not an assumed future branch.

| Area | What the code actually establishes | Refactoring consequence |
|---|---|---|
| Daemon spine | [Cargo targets/features](../../bins/eliot-searchd/Cargo.toml#L17) select entry.rs and default wave1-shell. [Entry dispatch](../../bins/eliot-searchd/src/entry.rs#L106) serves DIRECT/config/provider paths; the indexed compositions are not connected into source → Qdrant → validated result. | Complete concrete composition. Changing a feature default only changes compilation; it is not runtime wiring or readiness. |
| Local lexical index | [Legacy main](../../bins/eliot-searchd/src/main.rs#L5) owns [local postings](../../bins/eliot-searchd/src/lexical/mod.rs#L53). Cargo declares that source and six sealed CLI sources as test harness targets, not production binaries. | Remove obsolete harness implementations after mapping their shared modules and useful regression cases. Do not describe this as a demonstrated live production fallback. |
| Semantic fingerprints | [Eligibility digest](../../crates/search-query/search-access/src/lib.rs#L918) uses four noncryptographic lanes; related sites remain in eval, reclaimer, lexical fingerprints, retention and source records/views. | Replace each semantic producer in its owner, with an explicit preimage/profile and consumer disposition. Do not bundle all owners into one PR. |
| Eligibility collision example | The function returns EligibilityPlanDigest, not Blake3Digest32. Its inputs are membership/projection IDs, partition digests, route/epoch and security generations; the audit's collection_alias/namespace/partition string example does not describe this function. | The noncryptographic algorithm is confirmed. The audit's quoted trivial multi-field collision is not a demonstrated valid-input exploit. |
| Legacy point identity | [Root exports](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L350) still use a 16-byte non-S11 digest and ProjectionKind. [Planner](../../crates/search-index-qdrant/search-projection-planner/src/plan.rs#L18) and [publication](../../crates/search-index-qdrant/search-publication/src/model.rs#L6) already import s11. | Retire the unused root API under #329; preserve the implemented S11 API. Do not repeat the planner/publication identity port. A 128-bit Qdrant address remains correct; it is distinct from the required full 256-bit digest. |
| Legacy collision guard | [compare_existing_identity](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L439) and [PointIdentityRegistry](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L497) compare complete keys. The audit names a PointCollisionLedger that is not this implementation. | “Collision detection is mathematically impossible” is too broad. Full-key conflicts are detectable; the legacy hash/profile is still not S11 and should be removed. |
| Planner/publication progress | Planner verifies complete UnitSet membership and canonical manifests; [publication admission](../../crates/search-index-qdrant/search-publication/src/admission.rs#L388) is shared by [submit](../../crates/search-index-qdrant/search-publication/src/machine.rs#L143) and [restore](../../crates/search-index-qdrant/search-publication/src/machine/restore.rs#L95). | #259's algorithm port and #260's canonical admission have landed. Owner observations, actual effects and daemon serving remain separate integration work. |
| Qdrant bridge gap | [mutation.rs](../../crates/search-index-qdrant/search-qdrant-bridge/src/mutation.rs#L13) still defines QdrantPointId and a legacy PointPayload. It has not adopted the shared S9.5/S11 boundary. | #262 is real remaining work. Its old blockers #256/#258 have already landed. UUID length itself is not the defect; duplicate contracts and incomplete identity/schema validation are. |
| Reader allegation | The cited [direct_store::read_file_snapshot](../../bins/eliot-searchd/src/direct_store/store.rs#L366) is cfg(test). The reachable [--scan-file helper](../../bins/eliot-searchd/src/development/kernel/scan.rs#L112) reads a caller-selected diagnostic file under its parent. [Native admission](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs#L106) separately binds a registered root and policy. | Parent containment does not prove registered-root admission. Do not report this test helper or local diagnostic as proof that current registered-source ingestion bypasses its root. Do not reuse the diagnostic adapter as ingestion authority. |
| Windows identity/security adapter | [identity.rs](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/identity.rs#L47) has path-derived root identity and a fallback tuple. [open_handle](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/backend.rs#L95) also requires native checks before and after that branch. [read.rs](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/read.rs#L91) fixes barrier 1; inspect fixes Permitted. | These are inadequate inputs for authoritative ingestion. Persistent native-observation failure is not shown to pass; no root-escape exploit or sharing-violation fallback is demonstrated. Reuse the qualified native owner for admitted operations. |
| Synthetic deletion receipt | [confirm_absent_delete](../../bins/eliot-searchd/src/secret_composition/kernel/revocation.rs#L75) checks actual absence, then creates constant A5 evidence. No entry/secure-command/owner caller of this composer was found. | Compiled but disconnected composition debt. Delete it or bind evidence to the real effect/operation before activation. Do not claim the current daemon executed the fabricated receipt. |
| Fixtures/profile fallback | [FakeGuards](../../bins/eliot-searchd/src/publication_composition/kernel/guards.rs#L40), [build_test_export](../../bins/eliot-searchd/src/restore_composition/kernel/fixture.rs#L12) and the [zero-profile wrapper](../../bins/eliot-searchd/src/access_composition/grant_authority/admission.rs#L110) remain in composition source. Query serving uses the explicit for_profile variant; no production call to the zero-profile wrapper was found. | Remove unused wrappers or test-gate true fixtures; require real profile observations before activating a consumer. Presence in src alone is not runtime reachability. |
| Handwritten SHA-256 | Three sha256.rs implementations remain in daemon, document worker and Qdrant supervisor. Their APIs/protocols explicitly require SHA-256. | Reuse the accepted hashing owner/sha2 while preserving exact bytes. Replacing SHA-256 with BLAKE3, or adding framing to raw hashes, would corrupt compatibility. |
| Ledger | The ledger records 5,960 classified sites, including 1,770 test-vector entries. It is not a list consisting entirely of fake hashes. | Remove resolved migration/removal entries with their source change. Preserve valid product checks; do not grow exemptions or require every unrelated site to be repaired whenever a file is touched. |

### Digest repair boundaries

Prefer the existing [search-contracts digest owner](../../crates/search-contracts/src/digest.rs).
Canonical CBOR, domain-framed raw input and unframed exact bytes are different profiles. Preserve the
contracted algorithm, field order, framing, bounds and version; change identity only through an explicit
new profile/rebuild or an identified transient-plan invalidation boundary.

The FNV-derived u32 in [term_index_checked](../../crates/search-lexical/src/sparse/mapping.rs#L215) is a
sparse bucket mapping, not a cryptographic identity. S12.2 permits measured term collisions. Repairing
SparseFingerprint must not silently change term indexes or reinterpret previously built vectors.
A new term mapping needs its own qualified profile/generation.

The remaining owner map is #288 (eligibility), #233 (evaluation), #303 (index reclaim), #252
(lexical fingerprints), #302 (retention), #270 (source admission/view) and #272 (membership).
These are existing obligations, not newly issued source tasks. Persisted consumers must be identified
before choosing preserve-bytes versus new-profile migration.

| Producer | Output / consumer consequence |
|---|---|
| [Evaluation fingerprint](../../crates/search-eval/src/fingerprint.rs#L10) | FNV-like bytes wrapped as Blake3Digest32 feed report/run evidence. Identify historical report/profile compatibility before replacement. |
| [Reclaim plan](../../crates/search-index-qdrant/search-index-reclaimer/src/plan.rs#L107) | ReclaimPlanDigest is carried by receipts/checkpoints. Old work must be explicitly invalidated/replanned or migrated; an equal new field name is not replay compatibility. |
| [Sparse fingerprint](../../crates/search-lexical/src/sparse/fingerprint.rs#L13) | SparseFingerprint is separate from token indexes. Profile/evidence equality changes even if vectors remain numerically unchanged. |
| [Retention marks](../../crates/search-runtime/search-retention/src/sweep.rs#L774) | Protection/mark digests fence deletion. Resolve outstanding sweep intent/mark generations safely before switching profiles. |
| [Membership ID](../../crates/search-source/search-source-records/src/membership.rs#L215) | A 64-bit FNV value plus its complement becomes a SourceMembershipId; the stored batch decoder recomputes this legacy formula. In-place rehashing would break journal/reverse-index identity. Require a versioned key/reader migration. |
| [Workspace view ID](../../crates/search-source/search-source-registry/src/view.rs#L223) | Derived currentness token. Verify external cursor/reference retention before treating it as purely transient. |

## 3. Why the current instructions can reproduce the problem

These are causal risks supported by the instruction/code mismatch, not a claim that crate count alone
caused the defects.

- **Source presence substitutes for integration:** a pure owner can be complete while its real caller is
  absent. New facades and constructor-shaped proof do not complete that missing caller.
- **Historical issue bodies restart delivered work:** at review start, #259/#260 described old defects
  despite the planner port and PR #605. A blanket dependency on issue closure hides available APIs.
- **Unbounded cleanup contradicts bounded delivery:** “fix every exemption in a touched crate,” “delete
  every legacy family,” and “wire every module” turn a local repair into a multi-owner rewrite.
- **Old scheduling text competes with actual source:** the 10/09 launch/matrix and completion snapshots
  predate the present branch. They cannot restart #351/#354 or override the existing #598 manager.
- **The audit overstates evidence:** harness code, dormant composition, live diagnostics and admitted
  production paths need different treatment. Otherwise agents fix the wrong surface.
- **Micro-packaging became a delivery proxy:** Part I S5/S31 permits real capability boundaries, not a
  crate per type or forwarding facade. Remove duplicate state, codecs and obsolete paths first.
  Consolidation needs a demonstrated shared owner/dependency boundary, not an arbitrary crate-count target.

Do not weaken source truth, live deny/purge, unknown-outcome handling, one Qdrant backend, bounded
control storage or native identity to make a slice appear finished.

## 4. Small source slices using existing issues

These are preparation cards, not a new mandatory train. #352 still coordinates one source writer;
#598 retains its current work and unpublished WIP. No source branch is assigned by this report.
For each selected card, verify the current main and immediate callers, then record the existing
Definition of Ready. A whole row/issue is split again if it exceeds the existing review budget.

| Card / existing issue | Owner and bounded change | Required input / deletion boundary |
|---|---|---|
| R1 / #329 | search-point-identity: remove root legacy digest/key/registry APIs and their obsolete fixture imports; keep s11. | Current planner/publication already use s11. Census all callers; freeze the existing legacy encoded-key bytes as a test vector and preserve S11 rejection without the legacy producer. Retain a decoder only for an identified stored format and reader. No dual hashing, point relabeling or new identity implementation. |
| R2 / #262 | search-qdrant-bridge: adopt s11 PointId128 throughout the existing bridge identity API and necessary immediate consumer call sites. | Shared S11 is present. Delete QdrantPointId instead of adding a second public facade. Preserve private UUID conversion and full identity validation. Daemon query_composition.rs is an explicit reverse-consumer candidate, not permission for daemon-wide work. |
| R3 / #262 | search-qdrant-bridge: replace copied payload/schema/eligibility/epoch representation with search-contracts::indexed and private vendor conversion. | #258 is present; consume its exact 19-index baseline/types and integer-range boundary. Delete replaced field tables; do not introduce a second codec owner or change persisted generation semantics implicitly. |
| R4 / #262, then #263 | search-qdrant-bridge: bind operation dispatch to exact admitted generation/schema/artifact observations. | R2/R3 APIs; current control/supervisor observations. A process-local cache is not durable replay authority. Static admission and actual live verification remain distinguishable. Live effect wiring is a separate #263 slice. |
| R5 / #288 | search-access: replace derive_predicate_digest with one bounded canonical semantic preimage and real BLAKE3. | Inventory every EligibilityPlanDigest consumer/cache before fixing the profile. Keep request admission read-only. No registry, daemon or query-engine rewrite. |
| R6 / #316 | Existing shared hash owner plus the named materializer consumer: complete the bounded exact-stream BLAKE3 migration. | Preserve the accepted exact-byte profile and operation budget; do not buffer an entire large object or create another generic hashing framework. |
| R7 / #315 | Daemon SHA-256 and its exact callers: consume the shared bounded stream owner, preserving RAW/PARTS_V1 framing. | R6's stream mechanism as specified by the existing issue. Document-worker and supervisor replacements are separate owner slices. No blanket algorithm change. |
| R8 / #274 | Daemon access composition: delete the unused compile_standalone_pre_retrieval zero-profile wrapper and its re-export; migrate its remaining test callers to explicit profile inputs. | query_composition/serving.rs already uses compile_standalone_pre_retrieval_for_profile. Preserve that path and its live checks. No new profile issuer, query rewrite or production test constant. |

Other digest producers use the owner map above, each as a separate source repair. Disconnected A5,
FakeGuards, test-export and zero-profile wrappers are fixed or removed in their owning cutover, before
becoming reachable. A constant fixture in a test is not a production cryptographic producer.

For #267, preserve the existing qualified native acquisition. Any remaining diagnostic-adapter cleanup
must start from actual callers and its explicit diagnostic contract, not the audit's unproved ingestion
exploit. Do not duplicate #598's registered-root/body-custody work.

Legacy harness removal is a separate deletion sequence: map the seven Cargo test targets and shared
modules, preserve only meaningful regression inputs/cases, remove one obsolete family, and check its
remaining consumers. Do not issue a blanket deletion of sealed_*, snapshot or plaintext stores while a
current/legacy reader still uses them.

## 5. Actual product integration that remains

Preserve delivered S11 (#256), UnitSet (#257), indexed contracts (#258), planner algorithm/complexity
repairs (#259/#519), publication admission (PR #605), lexical operation-budget work (PR #608) and static
Qdrant artifact admission (PR #609). Their exact prior evidence is not a new runtime qualification.

Continue the existing source spine from #598: one admitted held source body → protected immutable object
→ durable occurrence/head/residency/retention link → exact reopen. Then advance the actual preparation,
ingestion and coherent-view consumers (#330/#331/#270/#271) using those outputs, without redoing their
already delivered substeps.

The remaining integration is composed from these existing owners:

| Boundary | Existing work | Concrete result |
|---|---|---|
| Publication | #261, #262/#263, #333 | Actual owner observations → durable intent → exact Qdrant effects/readback → guarded redb VisibleEpoch and route. Qdrant alias change is not the commit point (S13.4). |
| Source-backed query | #288/#274, #290/#291, #293/#294, #279/#280 | One coherent access plan → real retrieval → exact retained-source validation → bounded provider response. No post-filter-only access or source snippets taken from Qdrant. |
| Feature/startup cutover | Daemon integration under the existing programme | Connect baseline callers, remove obsolete facades, then make the required indexed capability set available in the normal build. DIRECT remains usable while Qdrant is unavailable; build features do not assert readiness. |
| Agent recipes | #213, #281 | Versioned free-text retrieval/orientation. Preserve existing v1 semantics; no raw Qdrant surface. |
| Qualification | #215/#264 and named owners | Integrated Windows/Qdrant/restart/fault/scale/installed evidence after the complete implementation exists. |
| Controller cleanup | #214 | Remove controller-only commands/metadata and their consumers. Preserve product validators; this cleanup does not block independent product implementation. |

These integration rows are programme boundaries, not single mergeable tasks. A selected slice names one
state/effect transition, exact existing inputs, immediate consumers, persisted-byte disposition and
what becomes reachable. Do not require completion of every crypto/lifecycle/optional profile first.

## 6. Implementation acceptance

Use one manager worktree and read/research/review subagents. Implement the selected causal result first.
Use the pinned Rust 1.98 toolchain and the smallest production closure:

    cargo +1.98.0 check --locked -p <library-owner> --lib --all-features
    cargo +1.98.0 clippy --locked -p <library-owner> --lib --all-features -- -D warnings

For changed daemon wiring, select its binary and the actual required feature set; inspect the normal
default and full-baseline build separately. --all-features also includes legacy-loopback-harness and
must not be presented as the normal product configuration.

Run no broad all-target/test/native/fault suites as a source-merge gate. Record required integrated
cases and execute them after the implementation is complete. New production diagnostics or a new
unsafe reachable path block; inherited unrelated debt is recorded once. Existing size debt does not
force a shrinking slice to repair an entire package.

Finish with one independent review of the final source head, checking actual callers, removed duplicates
and the stated compatibility boundary. No audit, ledger, label, mock result or successful compile
establishes product readiness.
