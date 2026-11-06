# Work register

Current checkpoint, 2026-11-06: Windows debug root operator qualified at source
ad8cc9e8fa498d43b644246af3b7f4900890317a. Source acquisition through verified Qdrant search remains unconnected.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Next task

**S1: connect the first real bounded file read to retained bytes and a guarded current source row.**
Use the actual daemon entry/app path and source owners: no-execute acquisition → plain ObjectRef →
guarded CurrentSourceRecord in the same search-control-redb 4.5.0 database. Preserve the acquired bytes through
retention instead of rereading them. Retained Source/materializer/Qdrant crates still need actual caller cutovers.
The [code map](../implementation/CODE_MAP.md#existing-dispatch-to-replace) names current entrypoints;
do not restore deleted root constructors or legacy fixtures.

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
| S1 / READY | F2 | daemon app + [source owners](../../crates/search-source/README.md#ownership), safe reader/object store/control | One actual bounded file read → ObjectRef → guarded CurrentSourceRecord. |
| P1 / WAIT | S1 | Materializer/unitizer and a real daemon consumer | One retained representation/ranges and explicit empty/gap outcomes. |
| Q1 / WAIT | P1 | Control, Qdrant bridge and publisher | Actual build/readback → matching current/evidence state → installed PublishedView. |
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
Current root operations use the same control/control.redb format through the control owner and redb 4.5.0;
the old 2.6 implementation is removed. The vendor data-safety patch exposes persistent-savepoint information
through the native read-only API. [Vendor provenance](../../vendor/redb/ELIOT_VENDOR.md) records that patch.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Verification and known blockers

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

Actual process-kill/power-loss recovery, other platforms, installed baseline, source ingestion, preparation,
Qdrant publication and verified search remain NOT_RUN/unconnected. [Quickstart](../../QUICKSTART.md)
contains only the qualified root operator surface. The overall project goal remains open.

## Update and finish a task

Update this checkpoint and its owning recipe with actual consumer changes and exact-source evidence.
Do not promote compilation, fixture success or a review badge into unexecuted product behavior.

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## History

Earlier investigation remains in [Git history](https://github.com/UnknownAlienHuman/eliot-search/blob/f680222357de64dcb131d50f2f4e5b3b825f493b/docs/audit/ANALYSIS_REGISTRY.md);
its deleted paths and former test/feature instructions are not current implementation work.
