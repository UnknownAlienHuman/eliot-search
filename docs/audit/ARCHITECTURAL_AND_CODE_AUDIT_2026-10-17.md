# ELIOT Search — architecture, code and donor audit

**Verification dates:** 2026-10-17–18. **Source baseline:** c8e38b6fc9829f1cd38070773b1b3e5b21de18ad,
the Gemini documentation commit over 239c572348dc6491f97de65464e889d41e25da80 (PR #609).
[Original Gemini report](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md).

This revision verifies and corrects that report; it does not assert that its proposed repairs have landed.
The [current local architecture](../architecture/README.md) now replaces the baseline obligations that
caused unnecessary migration, security and process machinery. This document records source evidence and
reduction decisions; it does not assign another writer or claim that the code has been changed.
Use [Implementation](../implementation/README.md) for implementation and
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
redb, ripgrep, Tantivy, Qdrant client, Serde/serde_json and Tokio source was read for concrete alternatives.
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
| Already delivered query mechanisms | [BlockingRealQueryPlane](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) owns one reused Tokio runtime; [evidence types](../../crates/search-contracts/src/evidence.rs) and [provider schema](../../crates/search-provider-protocol/src/frame/client/evidence_schema.rs) implement #213. | The older docs/comments calling these absent are wrong. Reuse and simplify #213's real model with its decoder/host; its existence does not require preserving all V1/V2 fields or bytes. |
| Local lexical index | [Legacy main](../../bins/eliot-searchd/src/main.rs#L5) owns [local postings](../../bins/eliot-searchd/src/lexical/mod.rs#L53). Cargo declares that source and six sealed CLI sources as test harness targets, not production binaries. | Remove obsolete harness implementations after mapping their shared modules and useful regression cases. Do not describe this as a demonstrated live production fallback. |
| Semantic fingerprints | [Eligibility digest](../../crates/search-query/search-access/src/lib.rs#L918) uses four noncryptographic lanes; related sites remain in eval, reclaimer, lexical fingerprints, retention and source records/views. | First decide whether the field and its consumer are useful. Remove obsolete plan/receipt fingerprints; do not turn them all into new cryptography. A surviving API that promises BLAKE3 must compute real BLAKE3 over its stated input. |
| Eligibility collision example | The function returns EligibilityPlanDigest, not Blake3Digest32. Its inputs are membership/projection IDs, partition digests, route/epoch and security generations; the audit's collection_alias/namespace/partition string example does not describe this function. | The noncryptographic algorithm is confirmed. The audit's quoted trivial multi-field collision is not a demonstrated valid-input exploit. |
| Legacy point identity | [Root exports](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L350) still use a 16-byte non-S11 digest and ProjectionKind. [Planner](../../crates/search-index-qdrant/search-projection-planner/src/plan.rs#L18) and [publication](../../crates/search-index-qdrant/search-publication/src/model.rs#L6) already import s11. | Retire the unused root API under #329; preserve the implemented S11 API. Do not repeat the planner/publication identity port. A 128-bit Qdrant address remains correct; it is distinct from the required full 256-bit digest. |
| Legacy collision guard | [compare_existing_identity](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L439) and [PointIdentityRegistry](../../crates/search-index-qdrant/search-point-identity/src/lib.rs#L497) compare complete keys. The audit names a PointCollisionLedger that is not this implementation. | “Collision detection is mathematically impossible” is too broad. Full-key conflicts are detectable; the legacy hash/profile is still not S11 and should be removed. |
| Planner/publication progress | Planner verifies complete UnitSet membership and canonical manifests; [publication admission](../../crates/search-index-qdrant/search-publication/src/admission.rs#L388) is shared by [submit](../../crates/search-index-qdrant/search-publication/src/machine.rs#L143) and [restore](../../crates/search-index-qdrant/search-publication/src/machine/restore.rs#L95). | #259's algorithm port and #260's canonical admission have landed. Owner observations, actual effects and daemon serving remain separate integration work. |
| Qdrant bridge gap | [mutation.rs](../../crates/search-index-qdrant/search-qdrant-bridge/src/mutation.rs#L13) still defines QdrantPointId and a legacy PointPayload. It has not adopted the shared S9.5/S11 boundary. | #262 is real remaining work. Its old blockers #256/#258 have already landed. UUID length itself is not the defect; duplicate contracts and incomplete identity/schema validation are. |
| Reader allegation | The cited [direct_store::read_file_snapshot](../../bins/eliot-searchd/src/direct_store/store.rs#L366) is cfg(test). The reachable [--scan-file helper](../../bins/eliot-searchd/src/development/kernel/scan.rs#L112) reads a caller-selected diagnostic file under its parent. [Native admission](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs#L106) separately binds a registered root and policy. | Parent containment does not prove registered-root admission. Do not report this test helper or local diagnostic as proof that current registered-source ingestion bypasses its root. Do not reuse the diagnostic adapter as ingestion authority. |
| Windows identity/security adapter | [identity.rs](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/identity.rs#L47) has path-derived root identity and a fallback tuple. [open_handle](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/backend.rs#L95) also requires native checks before and after that branch. [read.rs](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/read.rs#L91) fixes barrier 1; inspect fixes Permitted. | These are inadequate inputs for authoritative ingestion. Persistent native-observation failure is not shown to pass; no root-escape exploit or sharing-violation fallback is demonstrated. Reuse the qualified native owner for admitted operations. |
| Synthetic deletion receipt | [confirm_absent_delete](../../bins/eliot-searchd/src/secret_composition/kernel/revocation.rs#L75) checks actual absence, then creates constant A5 evidence. No entry/secure-command/owner caller of this composer was found. | Compiled but disconnected composition debt. Delete it or bind evidence to the real effect/operation before activation. Do not claim the current daemon executed the fabricated receipt. |
| Fixtures/profile fallback | [FakeGuards](../../bins/eliot-searchd/src/publication_composition/kernel/guards.rs#L40), [build_test_export](../../bins/eliot-searchd/src/restore_composition/kernel/fixture.rs#L12) and the [zero-profile wrapper](../../bins/eliot-searchd/src/access_composition/grant_authority/admission.rs#L110) remain in composition source. Query serving uses the explicit for_profile variant; no production call to the zero-profile wrapper was found. | Remove unused wrappers or test-gate true fixtures; require real profile observations before activating a consumer. Presence in src alone is not runtime reachability. |
| Handwritten SHA-256 | Three sha256.rs implementations remain in daemon, document worker and Qdrant supervisor. Their APIs/protocols explicitly require SHA-256. | Reuse the existing hashing owner/sha2 where these consumers survive. An external artifact's SHA-256 is not BLAKE3 or a hash of domain-prefixed bytes; replacing the algorithm alone would assert the wrong identity. |
| Ledger | The ledger records 5,960 classified sites, including 1,770 test-vector entries. It is not a list consisting entirely of fake hashes. | Retire the exhaustive per-site ledger/issue-state gate. Keep concrete hash/identity behavior tests and actual dependency checks; see §7. |

### Functional integration gaps

These are source-level findings at the inspected public commit, not reports of observed data loss,
production outages or completed Rust fixes. The [implementation guide](../implementation/README.md)
owns the coordinated changes; the table records why connecting existing constructors alone is insufficient.

| Source boundary | Confirmed behavior and consequence |
|---|---|
| [Native acquisition](../../bins/eliot-searchd/src/owner_composition/kernel/native_source.rs), [command result](../../bins/eliot-searchd/src/owner_composition/kernel/native_source_command.rs) | `acquire_source_v2` obtains `body`, hashes/classifies it, then returns `AcquiredNativeSource` without those bytes. The command records admission and reports `retained_revision:false`. No retained revision is produced by this handoff; pass the same acquired bytes to the writer. |
| [Registered-root sync](../../bins/eliot-searchd/src/owner_composition/kernel/source_sync.rs) | `run_source_sync_preflight` checks custody, registration and store, then returns an error. Even successful checks end in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`; this command cannot complete sync. |
| [UnitSet v3 input](../../crates/search-prep/search-unitizer/src/manifest/input.rs) | `prepare_unit_set_input` requires UTF-8, `ExactBytes`, an empty loss map, equal input/canonical digests and equal input/output byte counts. Existing materialization of transformed text is not automatically accepted; wider preparation needs real coordinate-map support, not relabelled assurance. |
| [Persistent transaction ledger](../../crates/search-control-redb/src/persistent/transaction.rs), [baseline limits](../../crates/search-control-redb/src/reference.rs) | Every fresh mutation retains an operation record and grows its cumulative byte count; no normal persistent pruning path was found. `plan_commit` rejects new mutations with `IdempotencyCapacityExceeded` at 65,536 receipts or when receipt bytes exceed 64 MiB, even if current records remain small. Completed bookkeeping becomes a lifetime write limit. |
| [Local abort classification](../../crates/search-control-redb/src/persistent/transaction.rs) | A staging `ReadCancelled`/`BudgetExceeded` becomes `CommitOutcomeUnknown` even when `write.abort()` succeeds before commit. This confuses a definite local abort with possible external writes; uncertainty belongs to the actual pending effect. |
| [Evidence contract](../../crates/search-contracts/src/evidence.rs), [result fence](../../crates/search-contracts/src/results.rs) | Retrieval/orientation validation requires nonzero exact-scan budgets; results require plan IDs/fingerprints and a result fence, while cards carry ranking traces. These obligations survive despite non-exhaustive top-k. Reuse useful scope/cards and remove unnecessary fields with their consumers; do not fabricate plans or proofs. |
| [V1 recipe host](../../bins/eliot-searchd/src/query_composition/registry.rs), [V2 wire schema](../../crates/search-provider-protocol/src/frame/client/evidence_schema.rs) | The host registers `RecipeIdV1`; #213 defines separate V2 evidence forms. Adding a name to the old registry cannot connect that request/result model. Decoder, dispatch and handler must change together; preserving eleven V1 byte fixtures is not a product requirement. |
| [Candidate readback](../../crates/search-query/search-candidate-validator/src/lib.rs) | `reopen_and_verify` compares digests supplied by `RevisionReadbackPort` to expected fields and checks range/length, but does not hash `readback.bytes`. A real reader must verify those bytes, or that check must move here. Matching supplied digests alone does not establish source truth; no live corrupt-reader incident is claimed. |
| [Daemon Publisher](../../bins/eliot-searchd/src/publication_composition/kernel/publisher.rs), [shared coordinator](../../crates/search-index-qdrant/search-publication/src/machine.rs), [floor CAS](../../crates/search-control-redb/src/publication_codec.rs) | Daemon `Publisher` duplicates publication floor/reservation/active state. `publication_codec::cas` validates arguments and returns the next floor; it performs no database write. Neither a state-machine transition nor a supplied commit observation is the real persistent publication boundary. |
| [Bridge receipt ledger](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/ledger.rs), [upsert](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/mutations/upsert.rs) | Replay receipts are retained in a process-local map; a full `max_operation_receipts` map rejects further mutations. It is neither durable restart recovery nor a substitute for the publication owner's pending operation. Remove the second replay owner when connecting that path. |

### Further code findings: repeated work and unusable state

The follow-up traced constructors to their callers and inspected donor source. These findings describe
conditions visible in source, not measured throughput or observed incidents. Shared indexed APIs remain
unconnected to production query serving; the public DIRECT paging and config paths below have actual
application callers. Detailed replacements live with the owners and in the implementation guide.

| Source boundary | Confirmed behavior → concrete change |
|---|---|
| [Startup config](../../bins/eliot-searchd/src/config_composition/kernel/process.rs#L184), [activation](../../bins/eliot-searchd/src/config_composition/kernel/activation.rs), [query field](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs#L242) | Startup compares a candidate against defaults and passes an empty `BTreeSet<ReceiptKind>` to `try_activate`. A permitted non-default `query.limit` requires `LiveApply` and returns `DAEMON_CONFIG_PARTIAL_REFUSED`; an authored test explicitly expects this refusal. Construct owners from validated typed settings instead of treating startup as hot reload. A supplied receipt **kind** does not execute a change. |
| [Config capture](../../bins/eliot-searchd/src/config_composition/kernel/process.rs#L93) | `vars()` converts all environment entries before the `ELIOT_SEARCH__` filter and can panic on unrelated non-Unicode data. `--config-file` needlessly requires Unicode. Metadata followed by `fs::read` can read/allocate past the cap if the file grows. Use one OS-string invocation, `vars_os`, and a bounded read from one opened file; remove capture/config hashes and the generic reload framework. |
| [Markdown verifier](../../crates/search-prep/search-materializer/src/markdown/pipeline.rs#L194), [JATS verifier](../../crates/search-prep/search-materializer/src/jats/pipeline.rs#L162), [structural UnitSet](../../crates/search-prep/search-unitizer/src/markdown_unitset/build.rs#L319) | Unitizers receive a private immutable materializer product but call `verify_*`, reopening and rebuilding raw text, maps and parsed inventory before constructing units. This is another full materialization, not a field check. Pass the validated product onward and verify persisted/external artifacts at their actual boundary. |
| [Revision immutable I/O](../../crates/search-source/search-revision-store/src/immutable_object/context.rs), [materializer I/O](../../crates/search-prep/search-materializer/src/legacy_artifact/context.rs#L913), [daemon persistence](../../bins/eliot-searchd/src/preparation_store/kernel/persist.rs#L143) | Two staging/write/sync/link/readback engines implement almost the same byte publication. A newly written plaintext preparation object is fully read in staging, after installation and again by the daemon. Consolidate one existing primitive and return its verified bytes/reference; preserve actual unprotect verification when encrypted mode is selected. |
| [Decode](../../crates/search-prep/search-materializer/src/decode/engine.rs#L136), [normalization](../../crates/search-prep/search-materializer/src/normalize/engine.rs#L83), [UnitSet codec](../../crates/search-prep/search-unitizer/src/manifest/codec.rs#L121) | Exact UTF-8 is copied across decode/preserve normalization; UTF-16 keeps per-scalar temporaries for line ends. UnitSet construction rebuilds a canonical tree and serialized bytes discarded after a size check. Move the original buffer, collect coordinates during the useful pass and encode only the persisted artifact. These are source-derived allocations, not measured peak RSS. |
| [Indexed candidate IDs](../../crates/search-query/search-retrieval-executor/src/indexed.rs#L282), [fusion key](../../crates/search-query/search-retrieval-executor/src/lib.rs#L501), [projector sort](../../crates/search-query/search-result-projector/src/lib.rs#L164) | IDs include `leg_id`, so the same point from two legs is not one fusion key. Projection then re-sorts by raw score and can lose the upstream fused order. Use one actual published evidence identity with its scope/revision/route semantics, then preserve the final order. Do not merge unrelated points because their bare IDs happen to match. |
| [Shared continuation resume](../../crates/search-query/search-continuation/src/lib.rs#L819), [completion scan](../../crates/search-query/search-continuation/src/emission.rs#L175), [DIRECT cursor](../../bins/eliot-searchd/src/continuation/kernel/catalog.rs) | Shared resume scans from the window's start and completion scans it again. For N unique items delivered one at a time, repeated prefix visits grow quadratically. The daemon already has a cursor and commit-after-delivery; preserve that mechanism in one surviving window owner. DIRECT does not repeat a full search or clone the whole window on next page; shared terminal transitions already drop their candidate payload. |
| [Public continue](../../bins/eliot-searchd/src/public_runtime_service/kernel/query.rs#L86), [continuation fence](../../bins/eliot-searchd/src/continuation/kernel/catalog.rs#L283), [handle preparation](../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs#L112), [source fence](../../bins/eliot-searchd/src/source_fence.rs#L10) | One normal page traverses `list_sources` three times: continuation fence, handle summaries and handle fence. Two traversals also serialize/hash the entire catalogue. Use the actual view and direct lookups needed for the bounded page; remove both duplicated catalogues/fences when consolidating, not add a hash cache. |
| [Handle quota/expiry](../../crates/search-query/search-handles/src/lib.rs#L437), [continuation pin release](../../crates/search-index-qdrant/search-epoch-pins/src/continuation.rs#L170) | Handle quotas count only Active entries while expiry/invalidation retain both maps, allowing unbounded terminal history. Pin `release` retains its binding; the success path does not call `retire_released`, so its 4,096-record ceiling can become a history limit. Remove released/expired state in the owning window lifecycle; a bounded active count alone does not bound memory. |
| [Executor cancellation](../../crates/search-query/search-retrieval-executor/src/lib.rs#L607), [handle expansion](../../crates/search-query/search-handles/src/lib.rs#L612) | Executor `cancel` clears outputs but does not own the separately returned `LegPinSet`; a retained ticket can still dispatch without the guard/deadline. Handle `max_expansion_bytes` is checked as a valid policy field but is not supplied to `revalidate`/`expand`. Make one operation own actual pins and pass the effective deadline/byte ceiling to the real reader/backend. |
| [Prepared publication](../../crates/search-index-qdrant/search-publication/src/machine.rs#L143), [compensation](../../crates/search-index-qdrant/search-publication/src/machine.rs#L388), [abandon](../../crates/search-index-qdrant/search-publication/src/machine.rs#L475) | `submit` takes the active slot and increments local reservation before durable intent. `Prepared` is excluded from both cancellation routes, and `complete` requires a published snapshot. Cancelling before intent can leave that coordinator busy until recreation. Keep a local discardable plan until the real pending write, or implement its plain pre-intent discard; no durable cancellation receipt is needed. |
| [Manifest codec](../../crates/search-index-qdrant/search-projection-planner/src/manifest.rs#L122), [prepared digest](../../crates/search-index-qdrant/search-publication/src/admission.rs#L501), [point output](../../crates/search-index-qdrant/search-projection-planner/src/model.rs#L143) | Canonical arrays cap one membership manifest at 4,096 entries despite larger declared point budgets; the shared codec caps bytes at 8 MiB. Prepared identity embeds/copies old and new manifests together. `PointSpec` keeps vector digests but not upload values. Use bounded artifact pages/ordered diffs, retain the actual encoder vector product for the write, and remove whole-manifest proof serialization. This is not a 4,096-point limit on the whole corpus. |
| [Retired manifest validation](../../crates/search-index-qdrant/search-index-reclaimer/src/manifest.rs#L71), [execution](../../crates/search-index-qdrant/search-index-reclaimer/src/execute.rs#L122), [guarded pin creation](../../crates/search-index-qdrant/search-epoch-pins/src/continuation/creation.rs#L25) | The validator compares caller-supplied digest/proof fields without binding them to the actual point IDs or reading committed retirement. Execution takes no deadline/cancellation/current pins. A one-time pin snapshot is insufficient while unguarded old-epoch/route admission remains possible. Read retired IDs from committed state, use the existing guarded query-pin lifecycle, and connect one bounded delete/readback operation. Another proof object would preserve the defect. |

### Digest repair boundaries

Delete unnecessary digest fields before repairing their algorithm. A plan, read receipt or review
catalogue does not earn a cryptographic identity merely because the old API has one. Remove the field
and its consumers, or use a simple in-memory identifier where that is all the operation needs.

For surviving source/artifact identity, reuse the existing
[search-contracts digest owner](../../crates/search-contracts/src/digest.rs). Canonical CBOR,
domain-framed raw input and unframed exact bytes hash different inputs. Compute the actual promised
algorithm over actual bytes. Change new formats with their real ends and rebuild derived generations;
an unsupported old root remains untouched without requiring a compatibility reader or migration engine.

The FNV-derived u32 in [term_index_checked](../../crates/search-lexical/src/sparse/mapping.rs#L215) is a
sparse bucket mapping, not a cryptographic identity. S12.2 permits measured term collisions. Repairing
SparseFingerprint must not silently change term indexes or reinterpret previously built vectors.
A changed term mapping requires rebuilding the affected indexed generation. This protects query/vector
agreement; it does not impose a qualification-token platform.

The following rows identify existing consumers, not mandatory hash-port or migration tasks.

| Producer | Output / consumer consequence |
|---|---|
| [Evaluation fingerprint](../../crates/search-eval/src/fingerprint.rs#L10) | FNV-like bytes wrapped as Blake3Digest32 feed report/run evidence. Remove unused attestation fields; an ordinary run identifier does not require backward compatibility or a cryptographic transcript. |
| [Reclaim plan](../../crates/search-index-qdrant/search-index-reclaimer/src/plan.rs#L107) | ReclaimPlanDigest is carried by receipts/checkpoints. Replace stale derived work with a newly computed plan; confirm actual live references before deleting data. No general replay-migration platform is needed. |
| [Sparse fingerprint](../../crates/search-lexical/src/sparse/fingerprint.rs#L13) | SparseFingerprint is separate from token indexes. Profile/evidence equality changes even if vectors remain numerically unchanged. |
| [Retention marks](../../crates/search-runtime/search-retention/src/sweep.rs#L774) | Protection/mark digests fence deletion. Resolve outstanding sweep intent/mark generations safely before switching profiles. |
| [Membership ID](../../crates/search-source/search-source-records/src/membership.rs#L215) | A 64-bit FNV value plus its complement becomes a SourceMembershipId; the stored batch decoder recomputes this legacy formula. In-place rehashing would break journal/reverse-index identity. Use a new format/root or a specific retained-data import; do not automatically require online migration. |
| [Workspace view ID](../../crates/search-source/search-source-registry/src/view.rs#L223) | Derived currentness token. Use the actual view/generation; bounded continuations can expire on restart. Do not preserve a cryptographic-looking token after its consumer disappears. |

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
  Docs saying they still need creating were stale. Simplify their obsolete inputs and connect the real
  end-to-end caller; delivery does not make the existing contract untouchable.
- **It kept review structure as an implementation target.** A one-owner quota, scope freeze, formal
  review record and source-layout assertion can all be satisfied while query composition stays dead.
  The new programme asks for useful behavior and deletion, with verification of concrete risks.

The [manifesto](../architecture/README.md) records the product principles; the
[implementation guide](../implementation/README.md#6-delete-replaced-mechanisms-and-consolidate) names
what replaces the mechanisms. These are not exceptions requiring another waiver or audit registry.

## 4. Mechanisms to retire or simplify

Counts are tracked physical newline counts at c8e38b6, not production LOC or promised net deletions.
Inline tests/comments/blank lines remain included. Areas overlap; do not sum them as a deletion total.

| Mechanism and source evidence | Actual state / cost | Decision for local search |
|---|---|---|
| [Daemon migration](../../bins/eliot-searchd/src/source_migration_command.rs), [cutover](../../bins/eliot-searchd/src/control_migration_cutover/operation.rs), [redb migration](../../crates/search-control-redb/src/migration/mod.rs) | Daemon `src` families: 33 files / 12,182 lines; redb migration: 31 / 10,976. Combined **64 / 23,158**, including 5,247 lines in 16 test-named files. Commands stage inactive artifacts. Cutover is uncalled and explicitly defers serving reroute; rollback is not a working reversal of an adopted store. | Retire general staging/cutover/rollback. New roots/generations rebuild from available sources. Leave unsupported old roots and sole revision copies intact; a separately selected reader/export is not a prerequisite for the new application. |
| [Revision crypto](../../crates/search-runtime/search-revision-crypto/src/lib.rs), [DEK selection](../../crates/search-runtime/search-os-secrets/src/revision_selection.rs), [active storage](../../bins/eliot-searchd/src/secure_direct_store/kernel/lifecycle.rs) | Active DIRECT storage uses RevisionProtector/Windows DPAPI. AES-GCM crate: 521 lines, no Rust import/dependency from a consumer. Five DEK candidate/selection/Windows/daemon files: **2,449 lines**. They publish/select metadata; no call connects the selected DEK to that AEAD crate. [Immutable object I/O](../../crates/search-source/search-revision-store/src/immutable_object.rs) exists, but native acquisition is not connected to one persistent revision writer/reader. A stale package contract must not be read as absence of all object I/O. | One consumed store design. New local storage may use ACLs + immutable verified snapshots. Encryption is selected for an actual confidentiality need; preserve existing encrypted roots/keys. Do not extend unused key machinery before a writer uses it. |
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

## 5. Integration and donor conclusions

The [implementation guide](../implementation/README.md) connects the existing source, preparation,
publication, Qdrant and query owners and names their replaced mechanisms. Preserve #598's manager and
useful source work. Delivered schemas/runtime are reusable code, not a reason to freeze their old contracts.

[Inspected upstream APIs](DONOR_VERIFICATION_REGISTER_2026-10-09.md#2-architecture-donors-read-in-source)
support short redb transactions, typed JSON, concrete dispatch, bounded Qdrant calls and one absolute
deadline. Serde is in Cargo.lock but is not integrated into the provider/control codecs. No donor is a
new search backend, a complete application to import or proof that the actual Eliot caller works.

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
