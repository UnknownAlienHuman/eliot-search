# ELIOT Search — architecture, code and donor audit

**Verification dates:** 2026-10-17–18. **Source baseline:** c8e38b6fc9829f1cd38070773b1b3e5b21de18ad,
the Gemini documentation commit over 239c572348dc6491f97de65464e889d41e25da80 (PR #609).
[Original Gemini report](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).

This revision verifies and corrects that report; it does not assert that its proposed repairs have landed.
The [current local architecture](../architecture/README.md) now replaces the baseline obligations that
caused unnecessary migration, security and process machinery. This document records source evidence and
reduction decisions; it does not assign another writer or claim that the code has been changed.
Use [EXECUTION_PROTOCOL](../product/EXECUTION_PROTOCOL.md) for implementation and
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) for the current manager handoff.

## 1. Result and review scope

The main problem is architectural: a single-user search tool was made dependent on a much broader
ownership, migration, key-lifecycle and evidence platform before its indexed query path worked.
Incomplete composition and duplicated mechanisms are consequences of that scope. The earlier preparation
preserved too many requirements and concentrated on small wrappers/documentation. This pass changes
those requirements and names the actual mechanisms to retire.

Reviewed: Part I, all ADR files (including the two differently scoped ADR 0003 files), root/package
instructions and execution maps; Cargo targets/features; daemon dispatch and relevant source-reader
paths; canonical/S11/planner/publication/bridge boundaries; the audit's digest sites; and current
#259/#260/#262/#329/#352/#598 context. This is a targeted static review, not a line-by-line review of all
Rust files. This pass also traces migration/restore, local root admission, revision encryption and key
selection, actual query/transport reachability, operation budgets and source-shaped tooling. Upstream
ripgrep, Tantivy, Qdrant client, Serde and Tokio source was read for concrete alternatives.
No compile, Clippy, test, Windows or live-Qdrant execution is claimed by this preparation.

### Measured size at the source baseline

| Measure | Exact result | Interpretation |
|---|---:|---|
| Cargo workspace members | 52 | 47 library-area packages, four binary-area packages, xtask |
| Tracked Rust files / physical lines | 1,593 / 393,383 | Includes comments, blank lines, tests and harnesses |
| Test-named Rust paths outside xtask | 359 / 101,276 | Filename-based inventory, not proof that all content is test-only |
| xtask Rust files / physical lines | 123 / 23,823 | Mixed product guards, historical maps and helpers |
| Remaining Rust files / physical lines | 1,111 / 268,284 | Includes inline tests, harnesses, comments and blank lines; not production LOC |
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
The three Rust categories above are disjoint: xtask first, then paths with a `tests` component or a
stem `test`/`tests`, starting `test_` or ending `_tests`, then the remainder. Do not infer that the
product itself contains 393k lines or that removing tests has simplified it. See the
[measured reduction work](#7-measured-reduction-work) for actual candidate boundaries.

## 2. Verified findings and corrections

Paths and line anchors below refer to the source baseline, not an assumed future branch.

| Area | What the code actually establishes | Refactoring consequence |
|---|---|---|
| Daemon spine | [Cargo targets/features](../../bins/eliot-searchd/Cargo.toml#L17) select entry.rs and default wave1-shell. [Entry dispatch](../../bins/eliot-searchd/src/entry.rs#L106) serves DIRECT/config/provider paths; the indexed compositions are not connected into source → Qdrant → validated result. | Complete concrete composition. Changing a feature default only changes compilation; it is not runtime wiring or readiness. |
| Already delivered query mechanisms | [BlockingRealQueryPlane](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) owns one reused Tokio runtime; [evidence types](../../crates/search-contracts/src/evidence.rs) and [provider schema](../../crates/search-provider-protocol/src/frame/client/evidence_schema.rs) implement #213. | The older docs/comments calling these absent are wrong. #213 is closed; connect serving rather than repeat a contract/runtime implementation. |
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
| Ledger | The ledger records 5,960 classified sites, including 1,770 test-vector entries. It is not a list consisting entirely of fake hashes. | Retire the exhaustive per-site ledger/issue-state gate. Keep concrete hash/identity behavior tests and actual dependency checks; see §7. |

### Digest repair boundaries

Prefer the existing [search-contracts digest owner](../../crates/search-contracts/src/digest.rs).
Canonical CBOR, domain-framed raw input and unframed exact bytes are different profiles. Preserve the
contracted algorithm, field order, framing, bounds and version; change identity only through an explicit
new profile/rebuild or an identified transient-plan invalidation boundary.

The FNV-derived u32 in [term_index_checked](../../crates/search-lexical/src/sparse/mapping.rs#L215) is a
sparse bucket mapping, not a cryptographic identity. S12.2 permits measured term collisions. Repairing
SparseFingerprint must not silently change term indexes or reinterpret previously built vectors.
A new term mapping needs its own qualified profile/generation.

Before repairing a hash producer, determine whether its plan/receipt/cache should exist at all. Delete
unused evidence plumbing with its consumers. Recompute transient state or rebuild projections after an
intentional format change; an unshipped legacy format does not automatically deserve a migration engine.
The following rows describe existing bytes and real risks, not a mandate to preserve every old profile.

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
| [Membership ID](../../crates/search-source/search-source-records/src/membership.rs#L215) | A 64-bit FNV value plus its complement becomes a SourceMembershipId; the stored batch decoder recomputes this legacy formula. In-place rehashing would break journal/reverse-index identity. Use a new format/root or a specific retained-data import; do not automatically require online migration. |
| [Workspace view ID](../../crates/search-source/search-source-registry/src/view.rs#L223) | Derived currentness token. Verify external cursor/reference retention before treating it as purely transient. |

## 3. What the previous preparation got wrong

- **It treated 8.4 Part I as untouchable.** S0/S2 exclude an authority service and multi-user tenancy,
  while S6.3/S7.2.1 impose six-domain residency and bilateral source-owner transfer. Protecting that
  whole requirement set was not a prerequisite for safe local search.
- **It optimized the bureaucracy.** Compressing repeated ledger hashes by 12% keeps the unnecessary
  per-site catalogue. The useful question is whether the gate catches a real behavior defect at all.
- **It preserved compatibility by default.** Migration staging has no serving cutover, yet its receipts,
  inventories and rollback forms became a programme of their own. The workspace version is `0.0.0` and
  the [GitHub release list](https://github.com/UnknownAlienHuman/eliot-search/releases) was empty when
  checked. This does not prove there are no user installations; it provides no evidence of a released
  compatibility promise either. Preserve actual user data and make the pre-release format policy explicit.
- **It missed delivered source.** #213 types/provider schemas and the blocking Qdrant runtime exist.
  Docs saying they still need creating were stale. The actual gap is the real end-to-end caller.
- **It kept review structure as an implementation target.** A one-owner quota, scope freeze, formal
  review record and source-layout assertion can all be satisfied while query composition stays dead.
  The new programme asks for useful behavior and deletion, with verification of concrete risks.

The [current architecture](../architecture/README.md#8-requirements-retired-from-the-local-baseline)
records the decisions. They are not exceptions requiring a new waiver or another audit registry.

## 4. Mechanisms to retire or simplify

Counts are tracked physical newline counts at c8e38b6, not production LOC or promised net deletions.
Inline tests/comments/blank lines remain included. Areas overlap; do not sum them as a deletion total.

| Mechanism and source evidence | Actual state / cost | Decision for local search |
|---|---|---|
| [Daemon migration](../../bins/eliot-searchd/src/source_migration_command.rs), [cutover](../../bins/eliot-searchd/src/control_migration_cutover/operation.rs), [redb migration](../../crates/search-control-redb/src/migration/mod.rs) | Daemon `src` families: 33 files / 12,182 lines; redb migration: 31 / 10,976. Combined **64 / 23,158**, including 5,247 lines in 16 test-named files. Commands stage inactive artifacts. Cutover is uncalled and explicitly defers serving reroute; rollback is not a working reversal of an adopted store. | Retire general staging/cutover/rollback. Use fresh-format rebuild; a specific non-destructive export only for actual retained data. Preserve source originals, sole revision copies, configuration and keys. |
| [Revision crypto](../../crates/search-runtime/search-revision-crypto/src/lib.rs), [DEK selection](../../crates/search-runtime/search-os-secrets/src/revision_selection.rs), [active storage](../../bins/eliot-searchd/src/secure_direct_store/kernel/lifecycle.rs) | Active DIRECT storage uses RevisionProtector/Windows DPAPI. AES-GCM crate: 521 lines, no Rust import/dependency from a consumer. Five DEK candidate/selection/Windows/daemon files: **2,449 lines**. They publish/select metadata; no call connects the selected DEK to that AEAD crate. C07's [contract](../../crates/search-source/search-revision-store/FUNCTIONS.md) says there is no CAS implementation yet. | One consumed store design. New local storage may use ACLs + immutable verified snapshots. Encryption is selected for an actual confidentiality need; preserve existing encrypted roots/keys. Do not extend unused key machinery before a writer uses it. |
| [Generic secret lifecycle](../../crates/search-runtime/search-os-secrets/src/lifecycle.rs) and [pairing composer](../../bins/eliot-searchd/src/secret_composition/kernel/composer.rs) | Lifecycle file plus composition subtree: 10 files / **2,789 lines**, including 411 test-named lines. No production construction of PairingSecretComposer was found. The [real pairing credential path](../../bins/eliot-searchd/src/access_composition/native_binding/opening/credential.rs) directly uses the Windows adapter. | Remove uninstantiated catalogue/lease/rotation composition. Keep concrete secret storage for Qdrant or a selected restricted-client/encrypted feature. |
| [Root snapshot](../../bins/eliot-searchd/src/owner_composition/kernel/native_snapshot.rs), [co-locks](../../bins/eliot-searchd/src/owner_composition/kernel/native_admission.rs), [request accounting](../../bins/eliot-searchd/src/owner_composition/kernel/native_request.rs) | Two owner slots, adjacent generation checks, current executable digest, two OS lock files, argv digest/entropy and per-native-call/canonical-byte accounting. `native_request.rs` alone has **1,097 lines**, including inline tests. The second lock excludes the old sealed owner. | One OS lock + root identity + transactional control state. Remove executable continuity and the second co-lock with the old writer. Keep real I/O/allocation/deadline limits, not generic per-call ceremony. |
| [Source admission](../../crates/search-source/search-source-admission/src/lib.rs), [native source output](../../bins/eliot-searchd/src/owner_composition/kernel/native_source_command.rs), [registry cutover](../../crates/search-source/search-source-registry/src/cutover.rs) | Admission package: 8 files / **9,992 lines**, including inline tests. Native-v2 command explicitly reports admission only, no retained revision/body serving. Namespace transfer APIs have no ordinary ingestion/query caller. | Explicit local roots, excludes, no-follow containment and stable read; one local namespace. Six-domain policy/transfer is not the baseline. Do not remove the real contained-reader boundary. |
| [Query registry/service](../../bins/eliot-searchd/src/query_composition/registry.rs), [executor](../../crates/search-query/search-retrieval-executor/src/indexed.rs) | Query composition family: 5 files / **1,643 lines**. Real query leg has no normal application caller. Executor unconditionally calls `count_exact` after nominations. | Concrete query/readback/validate/result flow. Ordinary top-k is explicitly non-exhaustive; count/proof becomes an explicit operation. Remove the implied `CompleteCandidateScope`/denominator contract with the count call. No new generic handler registry. |
| [Qdrant gate](../../crates/search-index-qdrant/search-qdrant-bridge/src/qualified/gate.rs), [connect](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/connect_schema/connect.rs) | `RealDataPlane::connect` requires QualifiedGate; its constructor checks a caller-supplied record of 13 probe names/booleans. A typed receipt cannot itself prove that probes ran. Actual endpoint/version/auth/schema checks are separate. | Keep live operational checks; move executed qualification reports out of runtime construction. Do not manufacture a passing receipt to bypass the current API. |
| [Optional restore composition](../../bins/eliot-searchd/src/restore_composition.rs) | 10 files / **937 lines**, including 44 test-named lines; wave-gated and no normal production caller found. S28.2 already calls paired restore optional. | Remove/defer accelerated restore. Rebuild projections; preserve actual registry startup restore and user-owned revisions. |
| [Custom Rust source checker](../../xtask/src/qdrant_boundary/source.rs), [module graph](../../xtask/src/qdrant_boundary/source/module_graph.rs) | Source-checker file/subtree: **16 files / 3,293 lines**, including 532 test-named lines (other regression files also contain tests). It reconstructs import/alias/module and public vendor-type reachability. Its documented unsupported Rust forms still need compiler/review. | Retire this partial analyser. Use existing Cargo metadata for dependency ownership, private Rust adapter visibility and review of the real API. Do not replace it with another taint framework. |
| [Source-shaped tests](../../bins/eliot-searchd/tests/rebuild_composition_module_ownership.rs) | **64 `*_module_ownership.rs` files / 7,965 lines**. The sampled rebuild test asserts module names, byte limits, string fragments and absence of transport tokens in the composition. This is an area inventory, not a claim that every assertion in all 64 files is useless. | Retire layout/token assertions that fossilize the implementation. Keep behavior/dependency boundaries that catch real regressions. |

The five DEK files are `revision_candidate.rs`, `revision_selection.rs`, Windows
`credential/revision_dek.rs`, and daemon `native_dek_command.rs`/`native_dek_selection.rs`.
The migration count excludes four daemon process-test files (1,678 additional lines); it includes
standalone tests nested in the two measured `src` families. No private #598 WIP was inspected.

### A concrete deadline defect hidden by all the accounting

The shared [OperationContext](../../crates/search-ports/src/context.rs) has a relative duration and a
cancellation-only `preflight`. The bridge's [OpContext](../../crates/search-index-qdrant/search-qdrant-bridge/src/real.rs)
also stores a `Duration`; [query](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/queries/search.rs),
[readback](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/queries/readback.rs),
[count](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/queries/count.rs) and
[scroll](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/queries/scroll.rs) each start a
fresh `timeout(context.deadline(), ...)`. Reusing this context does not enforce a total request deadline;
a sequence may consume several full per-operation durations unless a caller separately reduces them.
This is a source-level limit of the API, not a measured incident in the unwired public Qdrant path.

Convert the request deadline once to an absolute instant and pass the remaining budget/`timeout_at`
through real calls. Tokio already supplies this mechanism; no new budget framework is required.

## 5. The useful integration and donor conclusions

| Existing mechanism | Complete this consumer |
|---|---|
| Native held-source work / current revision preparation | Real retained snapshot → exact reopen → materialization/UnitSet. Keep #598's manager and reusable work. |
| S11/planner/publication plus actual Qdrant transport | One coherent contract/manifest → bounded real write/readback → committed route/epoch. |
| `BlockingRealQueryPlane` and delivered evidence schema | Real CLI/provider query → pinned scope → nominations → exact source validation → useful snippet/location. |
| DIRECT/exact scanning | Explicit exhaustive verification; don't force it into every top-k request. |
| redb + OS file lock | One local owner and committed state; reset/rebuild for incompatible derived formats. |

[Upstream source comparison](DONOR_VERIFICATION_REGISTER_2026-10-09.md#2-architecture-donors-read-in-source):
ripgrep shows one parsed invocation and concrete dispatch; Tantivy shows one writer/commit and a cheap
query snapshot; Qdrant's Rust example demonstrates direct client/upsert/filtered-query calls; Serde and
Tokio already provide serialization and absolute timeouts. These are mechanisms to reuse, not proposals
for another search backend or for copying a donor application's full architecture.

The full source-backed path is the implementation result. Orientation/comparison/documents extend it;
key rotation, provider ownership transfer and optional restore no longer precede it automatically.

## 6. Verification of the implementation

This preparation edits documentation/programme decisions. No Rust behavior has been changed or tested.
When the manager implements a source result, check the changed production owner and real consumers,
then test the integrated behavior that could fail: indexing/hit/readback, changed bytes, restart/index
loss, scope exclusion, cancellation and bounded output. Use live Qdrant/native evidence for those claims.

A fake callback, passed layout assertion, scope form, code count or review badge is insufficient. Preserve
real exact-byte/hash/collision/recovery behavior checks; retire tooling whose purpose is the old layout
or an exhaustive classification catalogue. Report remaining limitations and user-data disposition.

## 7. Measured reduction work

Counts below are source inventory at the same c8e38b6 baseline, not committed code deletion. Reduction
means an obsolete mechanism and its last consumers disappear; splitting files or moving them to a new
crate is not reduction. Preserve unique regression obligations with their actual owner. This preparation
changes documentation only and does not claim a sub-100k production implementation.

### Daemon harnesses: 52 files / 7,894 lines in three disjoint groups

[Cargo.toml](../../bins/eliot-searchd/Cargo.toml#L21) declares the old main and six sealed CLIs as test
targets. They are distinct from the current `entry.rs` product binary. Existing [#189](https://github.com/UnknownAlienHuman/eliot-search/issues/189)
can own bounded deletion slices after target/module/caller census:

| Group | Exact current inventory | Removal boundary |
|---|---:|---|
| Old snapshot/lexical/control prototype: `main.rs`, `control_store.rs` + directory, `lexical/`, `snapshot.rs` + directory | 16 files / 3,657 lines | Retire the old Cargo main harness and its local postings/two-file control persistence together. Current control authority remains search-control-redb. This is prototype retirement, not a production search-backend swap. |
| Six `src/bin/eliot-search-sealed-{authority,catalog,direct,recover,store,transaction}.rs` frontends | 6 files / 1,593 lines | Remove their six Cargo test targets and private entrypoints after identifying any retained fixture obligations. |
| Harness operations: `sealed_access.rs` + directory, `sealed_access_codec.rs`, `sealed_catalog.rs` + directory, `sealed_exact.rs`, `sealed_recovery.rs` + directory, `sealed_file_reader.rs` | 30 files / 2,644 lines | Follow the frontend removal; remove only modules whose last consumers were those targets. Catalog binding/rejection and recovery-report bounds have existing tests requiring an explicit retain-or-retire disposition. |

The old main's `self_test()` covers token/hex helpers, not a qualified indexed search spine. The sealed
[catalog tests](../../bins/eliot-searchd/src/sealed_catalog/kernel/tests.rs) cover binding bytes,
malformed identity before storage and receipt fields; [recovery tests](../../bins/eliot-searchd/src/sealed_recovery/kernel/tests.rs)
cover issue codes and bounded fail-closed reports. Retain the smallest cases only if those contracts
survive the harness. Do not keep whole obsolete implementations merely to host their tests, and do not
delete useful tests solely to improve a line count. Net removal may be less than the gross inventory.

**Excluded live/shared code:** [entry.rs](../../bins/eliot-searchd/src/entry.rs#L25) maps
`secure_direct_store.rs` to `direct_store` and the old `direct_store.rs` to `plaintext_direct_store`.
Revision/catalog/store portions remain reachable through secure storage, preparation, handles and
migration. `sealed_digest`, `sealed_owner_epoch`, `sealed_root_identity`, `sealed_root_lock`,
`sealed_store`, `sealed_transaction` and `sealed_transaction_guard` have current consumers. Names,
`allow(dead_code)` and cfg(test) on one helper do not authorize deleting these families or #598 work.

### CLI: existing client plus 2,031 lines of legacy candidates

The [CLI linkage map](../../bins/eliot-search/FUNCTIONS.md#current-source-and-missing-linkage) separates
delivered #235 session/native connection owners from the unconnected registration/listener/callers of
the paired native profile. Existing DIRECT already uses owned child stdio; the local baseline need not
wait for that optional restricted profile. Default dispatch rejects old endpoint commands;
`legacy-loopback-harness` is not the normal provider path. #332 connects actual supported local commands.

| Disjoint candidate group | Exact current inventory | Removal boundary |
|---|---:|---|
| `provider_client/core.rs`, `core/exchange.rs`, `core/exchange/cancellation.rs`, `core/response.rs`, `core/transport.rs`, `endpoint_client.rs` | 6 files / 1,638 lines | Replace the remaining old endpoint-engine callers with the existing typed owner; remove the engine and adapter with their last callers and explicit fixture disposition. |
| `remote_client.rs`, `app/indexed.rs` | 2 files / 393 lines | Retire these explicit legacy harness command entries when their last useful cases have a retained owner or an explicit retirement decision. |

These are physical source lines including comments/blank lines, not achieved net production deletion.
Mixed `provider_client.rs` retains typed exports/local assembly; mixed `app/core.rs` retains supported
DIRECT commands. Disconnected native registration/bootstrap is missing integration, not automatic
deletion credit. No second client, blanket DIRECT removal or invented trusted-registration shortcut.

### Tooling: remove derived state only after separating its consumers

Current [xtask dispatch](../../xtask/src/command.rs) contains validation/generation commands, not the
`agent_drafts`/ticket/lease issuer modules still described in the old #214 opening body. The remaining
opportunity is unnecessary policy and stored derived output, not deletion of all 23,823 xtask lines.

| Inventory | Size | Concrete disposition |
|---|---:|---|
| Architecture coverage, coverage graph and package-map source families | 49 files / 9,090 Rust lines | Mixed candidate area under #214; retain actual product invariants while removing duplicate historical policy and generated-map requirements. This is not 9,090 proven removable lines. |
| Entire `swarm/coverage` subtree | 205 TOML files / 152,921 lines / 5,146,354 bytes | Mixed reviewed inputs and generated outputs, not a deletion set. Architecture/capability/invariant/port/schema/recipe/delivery registries are loaded as validator inputs. They are not product source or 152,921 product defects. |
| Generated `swarm/coverage/package-maps/**` subset | 180 TOML files / 78,804 lines / 2,588,784 bytes | Concrete generator-owned output. Stop requiring committed maps before deleting them; change generator/validator/manual workflow consumers together. Remove obsolete policy inputs as well as outputs when their validator requirement is retired. |
| Qdrant, canonical-digest and integration-bootstrap source families | 35 files / 7,728 Rust lines | Keep actual Cargo dependency and runtime behavior checks. Retire the per-site digest gate and partial Rust source analyser; they are not untouchable because this row once called them product guards. |
| Cargo metadata adapter plus package status | 4 files / 2,214 Rust lines | Mixed shared dependency. Qdrant validation calls both [Cargo inventory and package-status validation](../../xtask/src/qdrant_boundary.rs#L125); separate actual Cargo/vendor enforcement from historical status policy first. |
| Accepted-evidence utilities | 3 files / 881 Rust lines | Actual consumer census required; do not call the entire family controller-only by its name. |

The Rust rows describe disjoint selected xtask families, not its complete total; the package-map data
row is a subset of the 205-file inventory. [Architecture loader](../../xtask/src/architecture_coverage/load.rs#L31)
and [package-map loader](../../xtask/src/package_maps/load.rs#L168) read reviewed inputs; the
[generator's write boundary](../../xtask/src/package_maps/generator/io.rs#L98) owns map outputs.
Other generated indexes/manifest fields can also participate in loading; identify those roles before
removal instead of treating everything under `swarm/coverage` as regenerable. Preserve needed CLI/common
helpers. [Package-map workflow](../../.github/workflows/package-map-coverage-v2.yml) and
[refactor-boundary workflow](../../.github/workflows/refactor-boundaries.yml) must change with their
derived-output contract; keep workflows manual-only. Retire obsolete input registries with the policy
that consumed them; being an input does not make a registry worth preserving. #214 covers the partial
Rust source analyser and map/status coupling; coordinate #525's ledger removal through the same manager.

### Ledger: retire the per-site classification gate

The ledger contains 5,960 sites across 833 paths and repeats 5,127 whole-file hashes. Its 3,572,618 bytes /
72,002 lines are an exhaustive metadata catalogue. The previous 12.08% normalization estimate addressed
representation, not the reason this structure exists.

[allowlist.rs](../../xtask/src/canonical_digest_guard/allowlist.rs) checks every site against the full
source-file SHA, count/test context, classification, owner issue snapshot, phase and a minimum review
reason. An unrelated edit in a shared file forces metadata churn; a completed owner can still be required
to appear OPEN in the offline snapshot. Neither fact verifies the runtime digest's algorithm/preimage.

Under #525, remove this per-site ledger and its issue-state/hash coupling from the gate and callers.
Keep concrete shared digest vectors, stored identity/profile behavior and actual dependency checks.
A scanner may remain an optional report if useful; do not recreate the catalogue as an allowlist or
one-hash-per-file approval database. #214 removes the separate historical map/status tooling.

### Reuse and consolidation

Three copied SHA-256 files total 685 lines; replace algorithm execution through the shared owner while
preserving framing (#315/#316 and separate worker/supervisor slices). S11, indexed contracts and the
reusable provider client already exist: #329/#262/#332 retire remaining copies rather than recreate
them. Markdown/XML/Unicode/redb already use mature libraries; their domain adapters are not duplicate
parser/database engines. Git's missing backend is completion work, not invented deletion credit.
Exact sources and limits are in the [donor index](DONOR_VERIFICATION_REGISTER_2026-10-09.md).

No new crate or generic API is justified by these counts alone. First remove the old state/codec/entry
owner. Consolidate surviving modules only where they share one real effect and have no independent
security, dependency or lifecycle boundary. Report achieved net changes and remaining callers in the
existing issue/PR; do not introduce a new LOC gate or documentation programme.
