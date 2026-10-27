# eliot-searchd — one application, one owner chain

Follow [architecture](../../docs/architecture/README.md) and the
[implementation spine](../../docs/implementation/README.md). Existing command/package/receipt boundaries are not
compatibility requirements. The [analysis registry](../../docs/audit/ANALYSIS_REGISTRY.md) records evidence and
unfinished checks; it is not a second specification.

## Current composition problem

The binary still contains several applications selected by an ordered interceptor chain:

```text
legacy effect refusal
→ registry/journal command
→ native-root lifecycle
→ migration
→ legacy preparation
→ DIRECT session
→ secure command stack
→ final shell
```

Each may capture argv/config and shadow later handlers. Feature names and compiled modules therefore do not prove a
connected product. Do not repair reachability by enabling all features or adding another dispatcher.

Representative current islands:

- source admission can read and hash a body but drops the owned bytes before immutable retention;
- source sync still ends in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`;
- preparation uses another DIRECT store/profile/receipt family;
- query/rebuild proof modules are not connected to the real Qdrant runtime;
- DIRECT query retains old continuation/result-handle and line-protocol systems;
- provider composition contains pairing/grant/envelope machinery unrelated to trusted child stdio;
- query/output can trigger corpus/history-sized storage inspection.

## R054 Windows publication correction

The active legacy source-registration route remains a concrete counterexample:

```text
secure_commands::maybe_run
→ source_registration::{--register-source-root | --unregister-source-root}
→ run_source_registration
→ registration_mutation::CatalogCustody::publish
→ registration_native::rename
→ SetFileInformationByHandle(FileRenameInfo) with relative leaf + non-null RootDirectory
```

R053 correctly rejected this wrapper as the new finalizer but incorrectly selected a private direct NT call as the
replacement. The current product uses neither path.

Selected owner operation:

```text
retain verified directory handles for every component from data root to final parent
  without FILE_SHARE_DELETE
retain opened stage handle with DELETE access
derive canonical absolute final path from retained final-parent handle
→ documented SetFileInformationByHandle(FileRenameInfo)
   RootDirectory=NULL; ReplaceIfExists=FALSE
→ reopen final relative to retained parent
→ verify stable identity, kind, length and bytes
```

The no-delete-share handle chain prevents rename/delete replacement of the absolute path while the documented call
runs. A focused canary must prove accepted path form, access/share flags, destination-present refusal, ancestor and
parent replacement resistance, supported local filesystem, failure debris and exact readback.

Disposition:

- do not reuse/repair the relative-root legacy wrapper;
- do not use path-based `MoveFileExW`/`tempfile::persist_noclobber` as authority;
- do not add private `NtSetInformationFile`/equivalent user-mode FFI;
- preserve only useful relative-open and file-identity techniques after current caller review;
- remove the legacy source-registration command route, mutation phases and wrapper during Foundation/source cut.

Passing old command tests cannot qualify the current finalizer. Until the documented canary runs, Windows root and
object publication remain unverified.

## Current Qdrant/query direction

```text
one explicit file
→ stable acquired bytes
→ plain ObjectRef + current SourceRecord
→ prepared retrieval_text
→ vectorless Qdrant 1.20 physical collection
→ explicit scoring Text index V1
→ InsertOnly complete build + exact readback
→ durable evidence/current commit
→ PublishedView install
→ TextQuery(k=1.2,b=0.75)
→ pinned evidence/ObjectRef/range
→ typed CLI output
```

Stage-2 text index is explicit: Code tokenizer, lowercase, no ASCII folding/phrase/stemmer/stopwords/token limits/
nested path, BM25 scoring, payload HNSW disabled, memory Pinned. Stage 2 creates no filter indexes. Query requests
no vectors and only evidence-routing payload.

Stage 2/3A uses complete immutable physical generations and deterministic generation-bound point IDs. Old/new
occurrence matching, stable point continuity, sequence and closures belong only to Stage 3B if release budgets fail.
Baseline cleanup deletes whole retired collections after guards and targetable attempts are gone.

## One target daemon

```text
startup:
  capture args_os/config once
  → explicit initialize or open current root
  → one process-lifetime DataRootGuard
  → construct current object/source/control/Qdrant owners
  → recover exact durable current state
  → install PublishedView and concrete handlers
  → one framed local dispatcher
ingest:
  selected source → stable read → immutable ObjectRef
  → conditional current SourceRecord → preparation outcome
  → complete Qdrant generation + readback
  → bounded evidence/current commit → view install
query:
  typed request → pin current view + resolve scope + RequestContext
  → TextQuery → point ID/routing payload
  → pinned evidence row → retained read
  → bounded terminal response → optional same-session QueryWindow
maintenance:
  stopped daemon + exclusive root
  → explicit local object collection or completed archive/import
```

An arrow is not recovery. Current input/outcome comparison repairs lost wake-ups. Pending external effects retain
only finite unfinished state. Normal reads and pure transforms create no durable receipt.

## Surviving responsibilities

| Owner | Responsibility |
|---|---|
| invocation/root | one settings value, initialize/open, documented tested finalization, one OS guard, bounded shutdown |
| source/object | stable acquisition; immutable bytes durable before current-head commit |
| current state | short typed redb rows/transactions; no history replay per operation |
| preparation | one materialization/unitization outcome with empty/gap/failure/stale guard |
| Qdrant | one private 1.20 runtime/client for publication and query |
| publication/view | fixed generation input, complete effects/readback, evidence/current commit and view install |
| query/evidence | direct TextQuery handler, pinned view/scope/context and retained read |
| session/window | one fixed-expiry final-order cursor for later Continue/Expand |
| protocol/CLI | framed local exchange, active Cancel and one owned-child lifetime |
| maintenance/import | stopped-root mark/sweep; completed archive; fresh destination/generation |

Responsibilities do not imply one crate each. Keep boundaries only for real dependencies/effects or substantial
algorithms.

## Removal order

Remove with actual consumers:

- current legacy source-registration command and relative-root wrapper;
- any direct-NT finalizer experiment introduced from R053 guidance;
- remaining argv interceptors and duplicate command implementations;
- registry/original-input replay and aggregate snapshots for ordinary source work;
- duplicate object/preparation stores and compatibility formats;
- migration/sealed/secure-command owner-slot/epoch/succession machinery;
- remaining wave features and optional old-package wiring;
- CLI/provider endpoint/token/pairing/grant/capability compatibility;
- query planner/ticket/access/proof systems and old rebuild/reclaimer composition;
- continuation/result-handle catalogues and recursive query-time storage inventories;
- fixtures whose only subject is removed topology.

Unsupported old roots fail before writes and remain untouched. Preserve original data and actual unpublished #598
overlap.

## Verification

After a Rust slice, run pinned Rust 1.98 check and strict Clippy for the changed production binary/owners and their
immediate consumers. `eliot-searchd --lib` does not cover binary composition.

Focused first evidence:

```text
Windows documented finalization canary
→ Foundation initialize/open/status
→ one retained file
→ Qdrant 1.20 schema/build/TextQuery canary
→ CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

Then exercise restart/edit/removal, incomplete inventory, old views, cleanup, cancellation, paging, relevance and
scale. Help, self-test, compilation and constructors are not application-level evidence.
