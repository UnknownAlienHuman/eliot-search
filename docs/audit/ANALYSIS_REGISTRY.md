# Work register

Current checkpoint, 2026-11-06: Windows debug root operator qualified at source
ad8cc9e8fa498d43b644246af3b7f4900890317a. S1 source 06bb6877487d7e3caac9d955be855c86dec1a1b5 connects one retained
file/current row and is qualified for the Windows debug S1 scope. P1 candidate d2ccb59638d9802d00933ee578a85f4f657a1980
connects complete stored preparation/current outcome and actual unit text. SOURCE review, source gate coverage
and bounded native cases passed; merge is pending. Exact binary scopes are below. Qdrant publication and
verified search remain unconnected.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Next task

**Q1: connect actual vectorless Qdrant publication after P1 merge.** Consume P1's stored representation,
bounded unit ranges and captured current outcomes. Follow the retained
[projection](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md) and
[publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) recipes: bounded uploadable text/pages,
actual collection/schema/readback, matching current/evidence commit and installed PublishedView.
The [lexical](../../crates/search-lexical/FUNCTIONS.md) and
[bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owners select server-side BM25 text
retrieval without a model/embedding provider. P1 Ready is preparation, not publication; U1 search remains later.

<a id="work-register"></a>
<a id="delivery-remaining"></a>
## Remaining work

The [package register](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#package-register) records package dispositions.
DONE below is the named delivered scope, not completion of the product.

| ID / status | Depends on | Owner / current starting point | Required result |
|---|---|---|---|
| C0 / DONE | None | Removed controller tooling and unused packages | Keep displaced producers/fixtures physically removed. |
| F1 / DONE, Windows debug root scope | None | CLI/daemon entry/app; [settings](../../crates/search-config/FUNCTIONS.md) | Consumed native file/environment/CLI root settings and effect-free help/version. |
| F2 / DONE, Windows debug root scope | F1 | daemon native_root_command; [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md) | Four plain root operations, stable exclusion, actual control format and bounded factual output. |
| S1 / DONE, Windows debug scope | F2 | current daemon/CLI + [source owners](../../crates/search-source/README.md#ownership), root, object store, control | One bounded file read → verified ObjectRef → guarded CurrentSourceRecord; final source 06bb6877 and distinct native scopes below. |
| P1 / VERIFIED, Windows debug scope; merge pending | S1 | Materializer/unitizer, shared object writer, CONTROL and actual prepare-source child consumer | Complete artifact/current outcome with actual stored text/ranges, empty/Gap and guarded CAS; source d2ccb596, distinct binary scopes below. |
| Q1 / NEXT after P1 merge | P1 | Control, Qdrant bridge and publisher recipes | Actual build/readback → matching current/evidence state → installed PublishedView. |
| U1 / WAIT | Q1 | CLI app and provider client/protocol owners | Owned-child framed search returning a verified retained excerpt. |
| R1 / WAIT | U1 | Source reconciliation, control and publication | Restart/inventory convergence and whole-generation lifetime/cleanup. |
| X1 / WAIT | R1 | Exact/query window and client owners | Retained exact denominator, same-session paging/expansion and expiry. |
| M1 / WAIT | R1 | Retention and future stopped-root maintenance caller | Conservative collection/archive/import preserving originals. |
| O1 / DEFERRED | Connected baseline + actual need | [Selected depth](../product/AGENT_ANALYSIS_FRAMEWORK.md#optional-depth) | Add only an actual source-backed consumer; remove unused scaffolds. |
| V1 / WAIT | Connected shipped paths | [Qualification](../implementation/TEST_CUTOVER.md), #215 | Measured relevance/scale/platform/release and installed-baseline evidence. |

[Implementation](../implementation/README.md#delivery-sequence) owns delivery order. Stage 3B is conditional
on measured Stage 3A budgets failing; there is no separate global rewrite programme.

<a id="established-decisions--do-not-rediscover"></a>
<a id="decisions-and-owners"></a>
## Preserved decisions

[Local scope](../architecture/LOCAL_SCOPE.md): one OS account, plain local storage, operator-owned same-host
Qdrant and owned-child stdio. Keep no-execute input handling, finite work, data integrity and conservative deletion.
Preserve user originals/keys and unpublished branches.

<a id="coverage-ledger"></a>
<a id="changed-documentation"></a>
## Cleanup delivered

The selected root/control/both-binary closures physically removed 705 obsolete files from base 6a7dfc1c to source ad8cc9e8, approximately 163,000 lines.
Legacy initializers/producers and exclusive fixtures are deleted, not retained behind cfg(test).
S1 separately records 138 deleted files against merged root base e4632d66; this is independent of the root's 705-file count.
P1 records 155 deleted files against merged S1 base 278a6b49, counted with --no-renames through d2ccb596, not net
file loss. Old preparation receipt/V3 APIs and their projection/publication admission, machine/restore callers,
exclusive fixtures and manifest edges were physically retired. Projection/publication FUNCTIONS.md and AGENTS.md
remain at their canonical paths as Q1 recipes; no old source/manifests were restored.
Current root operations use the same control/control.redb format through the control owner and redb 4.5.0;
the old 2.6 implementation is removed. The vendor data-safety patch exposes persistent-savepoint information
through the native read-only API. [Vendor provenance](../../vendor/redb/ELIOT_VENDOR.md) records that patch.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Verification and known blockers

P1 evidence scopes, Windows debug / Rust 1.98.0:

- Sol SOURCE review PASS at exact d2ccb59638d9802d00933ee578a85f4f657a1980, with no unresolved P1/P2.
  The only source delta from 88e00ef14fcabdfbc9dedc58cc5b47a33859c92a is CLI app request binding:
  response validation uses the already-parsed SourceId/unit, preserving legal native paths named like flags.
- Core runtime at 88e00ef, pinned B92E CLI/A74A daemon: original source deleted before first preparation;
  exact 153600-byte Unicode/CRLF/tab representation reconstructed from all ten complete CLI units and actual
  stored artifact. Empty produced a real zero-unit artifact; binary returned binary_content with no artifact/unit;
  a 169-byte script returned one unit without an execution marker. A 64512-byte quote/backslash line returned
  one complete unit in 129841 stdout bytes, within the derived 397312-byte bound. A separate changed-path case
  preserved original retained bytes; invalid UTF-8 returned its distinct invalid_utf8 Gap.
- Independent raw-artifact and native public CONTROL/storage/codec reads verified actual bodies, bindings,
  ranges and line flags. Missing/corrupt Prepared objects returned exit 2 OBJECT_MISSING/OBJECT_INTEGRITY_FAILURE
  without rebuilding, live-source fallback or CONTROL changes.
- The external CONTROL probe exited 0: pure read-only capture left bytes unchanged; actual settings/outcome
  A-B-A, expected absence-to-present and source-row CAS rejected stale captures. Identical settings/Ready
  preserved logical revisions; late Gap could not replace Ready. Prepared tags were rejected as Source.
  Constructed expected-value/registration mismatches were checked separately. Full historical
  None-to-present-to-None and re-registration transitions are NOT_RUN: no public clear/re-registration mutator
  exists. Writable rejection/no-op may change physical redb bytes; logical preservation is not byte identity.
- Patched d2ccb596 CLI C3D5 / unchanged A74A daemon: both relative roots literally --source-id and --unit
  changed from old exit 2 DAEMON_RESULT_UNKNOWN after durable Ready to exit 0, returning exact 40/35-byte
  stored units. Normal --unit 1 returned exact 16371 bytes [16371,32742), lines [153,306), matching stored
  descriptor/body and original slice. Core five fixtures were not rerun with the patched CLI.
- Focused prep tests passed 4 materializer + 8 unitizer tests. Actual final2 CONTROL/revision strict Clippy
  completed successfully. All six final4/post-P2 commands exited 0 with zero warnings:
  candidate-P1-binaries-final4-clippy.log, candidate-P1-daemon-final4-build.log,
  candidate-P1-cli-final4-build.log, candidate-P2-cli-check.log, candidate-P2-cli-clippy.log and
  candidate-P2-cli-build.log. The separate
  cargo +1.98.0 clippy --locked -p search-source-registry --lib --no-deps -- -D warnings at d2ccb596 exited 0;
  its log is candidate-P1-registry-final-registry-clippy.log. Source gate coverage is complete.
- Native handoff found all 53 recorded processes ended, no owned process/marker, and five golden fixtures
  unchanged. Full commands, hashes, PID/exit/output, byte/range comparisons and NOT_RUN boundaries are in
  C:\Development\Rust\targets\task-p1-native-fixtures\run-20261106-v1\NATIVE-RESULT.md and .json.
  Build/check logs are in C:\Development\Rust\targets\eliot-search-product-runtime.

Pinned P1 artifact SHA-256 values:

    core 88e CLI  B92EBCC9330177AF199DFE642F402416082226FB3B67A805E12BBB75EFEAD194
    d2 CLI        C3D50A4CE8379ACE04C03627F37FD189098139B2E06B3F7F3A8AD81DC602099A
    daemon        A74AAD50C5D8C0F6D2C31612A9828AE5EC289F8EA8EE6414B3C0BEA0159955D6

S1 evidence scopes, Windows debug / Rust 1.98.0:

- Final source 06bb6877487d7e3caac9d955be855c86dec1a1b5: all seven locked checks exit 0 without warnings,
  all seven strict Clippy runs (--no-deps -- -D warnings) exit 0 with zero diagnostics, and actual daemon/CLI
  builds exit 0. Targets: search-safe-reader, search-revision-store, search-source-registry,
  search-control-redb and search-runtime-owner --lib; eliot-search --bin eliot-search and
  eliot-searchd --bin eliot-searchd. Commands use cargo +1.98.0 check/clippy/build --locked with those
  explicit production targets. Sol SOURCE review PASS at this exact source, without unresolved P1/P2.
- Final 06bb CLI basic execution used a new root: independent 51-byte Unicode readback, repeated row 1 → 2
  with the same SourceId/digest, a copied path with a distinct SourceId and shared object, and different-path
  v2 bytes with a new object while the old object stayed unchanged. Empty/11-byte binary/160-byte script
  retention was exact; the execution marker stayed absent. Missing file returned SOURCE_OPEN_FAILED/exit 2;
  owned children were reaped. This final basic run does not relabel the earlier deeper cases as 06bb execution.
- CLI/daemon native binaries at 2eda3017c5ae650efcf797c2aa5a18bf2da21fe5 passed same-path revisions 1 → 2 → 3
  with stable SourceId, a distinct path with identical bytes and its own identity, exact empty/binary/script
  inputs with no execution marker, held-writer refusal, missing/oversize errors, junction refusal and savepoint
  preservation. Failed source observations preserved root/control/object snapshots.
- An external native library probe at cd76d68473e2580308fb8690e07b8a81d471b874 executed 29 calls / nine acceptance
  cases: independent raw CONTROL-row and retained-BLAKE3 verification; actual stale row and registration CAS;
  retained reading after live-source mutation; and missing/corrupt-object errors without live fallback or repair.
  Missing objects returned OBJECT_MISSING; same-length corrupt objects returned OBJECT_INTEGRITY_FAILURE.
- Stale SOURCE_STALE preserved the newer logical current row/object; the rejected candidate object remained
  unreferenced. Physical control.redb bytes changed during the writable stale-CAS transaction. Registration-only
  coverage used a direct external fixture transition, not a product unregister/re-add operation.
- Fresh read-only capture, unknown source schema, corrupt namespace and missing CONTROL observations caused no
  file changes. The file-symlink case is NOT_RUN (Windows privilege error 1314); a directory-junction refusal
  was actually exercised. Forced uncertain-commit I/O failure, kill/power-loss recovery, other platforms,
  installed baseline and product unregister/re-add remain NOT_RUN. No owned subprocess remained at handoff.

The final 06bb binary SHA-256 values are:

    CLI     28BE170778A6E6AD287A55B15F34276DCB797DE549648FA42747CE2A59F091EA
    daemon  5A486165D87FE38474A5126E10FB20EBFC50308DAD99C79260241887022E09DE

The final CLI hash matches the pinned 2eda CLI artifact. Deeper native library cases remain cd76 probe
evidence, not final-binary reruns. All claims above name source revisions, not this later documentation head.

Evidence is for the following exact SOURCE revisions on Windows debug / Rust 1.98.0, not a later docs head:

- f680222357de64dcb131d50f2f4e5b3b825f493b: locked check and strict Clippy (--no-deps -- -D warnings)
  PASS for search-control-redb --lib, search-runtime-owner --lib, eliot-search --bin eliot-search and
  eliot-searchd --bin eliot-searchd; both binary builds PASS.
- ad8cc9e8fa498d43b644246af3b7f4900890317a: CLI-only deadline-boundary delta; CLI locked check, strict
  Clippy, build and new basic execution PASS. Root/control/daemon blobs are unchanged from f680.
- At ad8, 35 native command records / 22 recovery-config assertions PASS. Real formatted and vendor preformat-empty
  retry reached current; discard retained only the same root.lock native file identity. Known partial stages
  recovered; completed hardlinks classified current. Unknown stages/sentinels refused with inventories preserved.
  File-parent versus startup-cwd resolution, file < environment < CLI across distinct roots, and overridden
  empty/escaped-NUL lower-layer rejection passed. Stages were constructed filesystem states, not process kills.
- Same golden savepoint ID 1 / retained-user-data regression: old 248 was RED (database deletion on a copy);
  fixed f680 CLI and daemon init/discard refused and preserved every file hash.
- Earlier native root API probes passed actual second-process Busy and cancellation/expiry before initialization
  with no filesystem effects; these were not repeated in the ad8 batch. The f680 Windows descendant-pipe
  deadline case passed once in 180.75 seconds. Actual expired-before-spawn process execution: NOT_RUN; source
  boundary reviewed. No universal OS timing guarantee is inferred.
- Independent vendor read-only source/API baseline and final ad8 Root SOURCE review by gpt-6.1 Sol PASS;
  no unresolved P1/P2 findings. Source review and helper-native owner evidence are not installed-baseline proof.

The ad8 recovery/config artifact SHA-256 values are:

    CLI     473868F3DD18A6D30CE58B6AC19AFEC16E88405891E17603E124134EDA860D3D
    daemon  F71ABD97C1A051709D8F7E8871139EED2F153900A044FF850932C024D60E03D0

Actual process-kill/power-loss recovery, other platforms and installed baseline remain NOT_RUN. Qdrant publication
and verified search remain unconnected. [Quickstart](../../QUICKSTART.md) names root, retained-file and retained-only
preparation commands. S1/P1 evidence is limited to the scopes above; the full product goal remains open.

## Update and finish a task

Update this checkpoint and its owning recipe with actual consumer changes and exact-source evidence.
Do not promote compilation, fixture success or a review badge into unexecuted product behavior.

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## History

Earlier investigation remains in [Git history](https://github.com/UnknownAlienHuman/eliot-search/blob/f680222357de64dcb131d50f2f4e5b3b825f493b/docs/audit/ANALYSIS_REGISTRY.md);
its deleted paths and former test/feature instructions are not current implementation work.
