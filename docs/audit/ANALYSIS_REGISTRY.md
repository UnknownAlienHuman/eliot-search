# Analysis registry

Continuation memory and evidence status. It records inspected facts, selected decisions, published changes and
unrun work; it is not architecture, permission or an implementation prerequisite.

<a id="resume"></a>
## Resume

**Checkpoint:** R054, 2026-10-26 — R053 private-NT finalizer overreach corrected.  
**Branch:** `codex/refactor-preparation-20261017`, PR #610.  
**Inspected ELIOT head:** `9910a15bf34a6d5417da8e45eac9a3f124733f11` (R053).  
**Original source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.

Read [docs/README](../README.md), the [implementation sequence](../implementation/README.md#delivery-sequence),
then only the affected owner recipe, current callers and owning issue.

R054 changes documentation only. Product remains unimplemented. The next implementation remains Foundation, but
the Windows finalization canary now uses a documented Win32 absolute-target operation protected by a retained
no-delete-share handle chain. It does not use private NT user-mode APIs.

## R054 inspected evidence

### Upstream platform material

Inspected:

- Microsoft `FILE_RENAME_INFO` and `SetFileInformationByHandle` documentation;
- Microsoft `CreateFile`/`CreateFile2` share-mode semantics: omitting `FILE_SHARE_DELETE` prevents subsequent
  delete-access opens, and delete access covers rename;
- current supported-build report reproducing `ERROR_INVALID_PARAMETER` when the Win32 wrapper is given both a
  relative destination and non-null `RootDirectory`, while absolute destination + null root succeeds;
- Microsoft driver/DDI material for `FILE_RENAME_INFORMATION`, showing that direct NT file-information APIs are not
  the ordinary documented desktop Win32 contract;
- current R053 root/object/daemon instructions and reachable legacy wrapper route.

### ELIOT source/docs checked

- root `AGENTS.md`, `README.md`, `docs/README.md`;
- implementation spine;
- root owner and immutable object-store recipes;
- daemon composition map;
- donor register;
- PR #610 and active root/object/programme issues.

## Findings and decisions

### F1 — R053 replaced one unqualified wrapper with another unqualified API boundary

R053 correctly rejected:

- path-based `MoveFileExW`/`tempfile::persist_noclobber` as trusted authority;
- the reachable legacy relative-`RootDirectory` Win32 wrapper.

It then selected a direct `NtSetInformationFile`-style user-mode operation. That is not the documented Win32
application contract and would require private ntdll ABI/support assumptions. This was an architectural error.

**Decision:** remove private direct NT rename from the selected baseline.

### F2 — documented absolute-target rename can be made path-stable without a private API

The root owner already needs retained directory handles. Windows share-mode documentation states that omitting
`FILE_SHARE_DELETE` prevents later opens requesting delete access; delete access includes rename.

**Decision:** retain and verify every directory component from data root to final parent without delete sharing,
retain the stage handle with DELETE access, derive one canonical absolute final path from the retained final-parent
handle, then call documented `SetFileInformationByHandle(FileRenameInfo)` with:

```text
RootDirectory = NULL
ReplaceIfExists = FALSE
```

Reopen final relative to the retained parent and verify stable file identity plus exact content. The handle chain
prevents rename/delete replacement of the path components while the absolute path is resolved and used.

### F3 — relative RootDirectory remains documentation-versus-runtime evidence, not a baseline

Official `FILE_RENAME_INFO` documentation describes a relative destination with `RootDirectory`. A current supported-
build reproduction shows the wrapper may convert the name to absolute and leave the root handle set, producing
`ERROR_INVALID_PARAMETER`.

**Decision:** do not use that form until a supported-platform canary demonstrates it. Do not bypass it with private
NT APIs. The documented absolute-target form is the only selected Windows candidate.

### F4 — support is a canary result, not a prose assertion

Before Windows Foundation is advertised, execute:

- canonical path derivation from retained parent and accepted extended-path form;
- stage DELETE access and exact share modes;
- retained root→parent chain without `FILE_SHARE_DELETE`;
- absent destination success and existing destination refusal;
- attempted ancestor/parent rename or replacement during operation;
- local same-volume filesystem support and unsupported-profile refusal;
- failure/crash debris classification;
- relative reopen, stable file identity and complete byte verification;
- truthful file-data, namespace and sudden-power-loss durability claims.

Failure means the platform profile is unsupported. There is no MoveFileEx, relative-root-wrapper, private-NT or
generic-filesystem fallback.

## Preserved Qdrant decisions

Stage-2 physical Text index remains explicit:

```text
vectorless one-shard collection
retrieval_text Text index
Code tokenizer; lowercase=true
ascii_folding=false; phrase_matching=false; stemmer disabled
stopwords/min/max/nested absent
BM25 scoring
enable_hnsw=false
memory=Pinned
```

Query profile remains k=1.2, b=0.75, finite/range validated, no threshold. Stage 2 creates no filter indexes,
requests no vectors and only routing payload, and requires exact schema/resource/point/count readback plus a
deterministic TextQuery canary. Repository lock remains client 1.19; implementation upgrades all 1.20 pieces
together.

## Current implementation sequence

| Stage | Observable result | Primary owners |
|---|---|---|
| 1 Foundation | documented Windows finalization canary; root/plain settings; manifest-last initialize/open; one guard; status | #333/#266/#360 |
| 2 Walking skeleton | one file → ObjectRef → retrieval text → explicit Qdrant 1.20 Text index → TextQuery canary/query → retained CLI excerpt | #264 and vertical owners |
| 3A correctness/reference | restart; inventory; explicit filter/residency profile; complete generation/view; budget matrix | #270/#271/#331/#261/#300/#303 |
| 3B conditional optimization | source-local diff/identity/mutation only where 3A budgets fail | #259/#329/#261/#263 |
| 4 Exact/session | retained-source Exact and one framed live QueryWindow | #249/#275/#280/#332 |
| 5 Maintenance/depth | stopped-root maintenance and selected product leaves | #302/#244/#281/#227/#335 |

## Anti-bureaucracy and lock rules

- Issues are owners, not dependency order.
- One manager owns each vertical cut; parallel writes are disjoint.
- No hidden server defaults, backend abstraction, alias manager, custom-shard fusion, watcher ledger, init journal,
  query scheduler/tickets, lexical qualification receipts or operation archive.
- No platform framework: one private current root/object finalization seam only.
- No redb writer/view mutex/window mutation borrow spans object/Qdrant/process/model/filesystem publication I/O.
- Complete build + schema/canary + durable evidence commit precede view installation.
- Last view drop only wakes cleanup.
- Current-caller search is local to code moved/deleted.
- Minimal Rust 1.98 check/Clippy follows production cuts; broad evidence follows connected behavior.

<a id="evidence-status"></a>
## Evidence status

| Area | Status |
|---|---|
| Legacy relative-root wrapper route | CHECKED reachable by static caller trace |
| Relative RootDirectory Win32 form | DOCUMENTED upstream but REJECTED pending supported-platform canary |
| R053 direct private NT candidate | REJECTED as baseline |
| Documented absolute-target Win32 candidate | SELECTED CANARY, not implemented/executed |
| Retained no-delete-share path chain | SELECTED design, not executed |
| Exact Qdrant physical text profile V1 | SELECTED, not compiled/executed |
| Qdrant client/server upgrade | NOT DONE; lock remains 1.19 |
| Compiler evidence | R031 materializer + unitizer only |
| Foundation/walking skeleton/current binaries | UNRUN |
| Windows durability/live Qdrant/restart/relevance/scale/end-to-end | UNRUN |

No static donor/source inspection proves correctness.

## Immutable prior records

- R001–R031: audit/removal/dependency work through `da382b66`.
- R032–R045: canonical documentation reconciliation through `af337ec7`.
- R046: implementation-readiness correction `1eeb4191`.
- R047: active-surface hostile verification `858d1e34`.
- R048: donor/analogue synthesis `fed10c1e`.
- R049: Qdrant 1.20/platform-finalization correction `9edd2c1d`.
- R050: active Qdrant-owner synchronization `a4c23110`.
- R051: exact Qdrant query/schema reconciliation `87c84c24`.
- R052: explicit Qdrant text-index resource/query semantics `8942d823`.
- R053: active Win32 wrapper trace and direct-native selection `9910a15b`.
- R054: private-NT rejection and documented handle-locked Win32 finalization correction.

## Next action

Implement the smallest documented Windows finalization canary in the current root/object owner. Run minimal
compiler/Clippy and focused path-lock/finalization cases. Then complete Foundation, upgrade the exact Qdrant
client/server/schema/query profile and implement #264.
