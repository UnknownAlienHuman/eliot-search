# Current dependency and lifetime graph

This graph records data/effect ownership for implementation. It is not a package-count target or agent-control
plane. The [implementation spine](../implementation/README.md) owns sequencing; nearest `FUNCTIONS.md` owns local
seams.

## Baseline data path

```text
Invocation / BootstrapSettings
        │
        ▼
DataRootGuard ──owns lifetime──► redb control + plain object namespace
        │
        ▼
stable source acquisition
        │ owned exact bytes
        ▼
PlainObjectStore ──► ObjectRef
        │                  │
        │                  ├──► current SourceRecord (short redb compare/set)
        │                  └──► retained reads for preparation/evidence/Exact
        ▼
materialize + unitize ──► PreparedRef/outcome
        │
        ▼
serialized Publication
        │ fixed PendingGenerationBuild
        ▼
Qdrant 1.20 physical generation
  scoring full-text payload index + code tokenizer
        │ exact settled readback
        ▼
complete evidence/inventory/current redb commit
        │
        ▼
open matching read snapshot
        │
        ▼
short PublishedView install
        │
        ├──► TextQuery Search against exact physical collection
        └──► EvidenceRef → immutable ObjectRef → verified excerpt
```

Qdrant is derived retrieval state. Immutable objects are source evidence. Redb stores compact current bindings and
real read snapshots, not source bodies or Qdrant payload pages.

## Foundation finalization

```text
retained root/final-parent handles
+ opened stage handle
+ final leaf
        │
        ▼
owner-private no-clobber finalizer
        │
        ▼
ordinary reader exact verification
```

On Windows this is a handle-based `SetFileInformationByHandle` family operation after a focused canary.
`tempfile::persist_noclobber` is not trusted as the finalizer because its current Windows implementation is
path-based `MoveFileExW`. On Unix use a directory-relative no-replace primitive or verified link/unlink fallback.
Root and object owners may share a tiny platform module; no generic filesystem framework.

## Publication profiles

### Stage 3A — correctness/reference

```text
complete desired state
→ fresh never-reused collection
→ complete insert_only build
→ complete evidence/current commit
→ explicit PublishedView install
→ old generation retained by guards
→ whole-generation cleanup
```

A source change creates a new complete generation. Cross-generation point continuity and `valid_until` are absent.
This profile ships for a corpus class only after declared latency/throughput/disk budgets pass.

### Stage 3B — required only when budgets fail

```text
bounded changed-source delta
→ unambiguous occurrence reuse
→ insert_only additions + fixed non-creating closures
→ complete-source evidence/current commit
→ measured compaction to fresh generation
```

Do not introduce Stage 3B before Stage 3A measurements. Custom shard keys are not a generic substitute because
BM25 statistics are shard-local and cross-shard portfolio ranking would need another fusion owner.

## View and cleanup ownership

`PublishedView` owns:

```text
exact physical collection name
retrieval profile
real redb read snapshot
generation-local metadata needed by query
```

Qdrant aliases are not serving authority. Query clones the current view under a short lock, then releases it before
scope, backend and object I/O. A live `QueryWindow` may retain that guard.

Retired collection deletion requires no durable/current/pending target, no live guard and no targetable unresolved
attempt. Last guard drop only wakes cleanup. Delete and exact absence observation occur outside redb/view locks.

## Lock and effect order

```text
process lifetime DataRootGuard
    └─ concrete stores/runtime

publication permit (logical serialization only)
    └─ fixed input read
       └─ object/Qdrant I/O without redb writer/view lock
          └─ bounded redb commit
             └─ open snapshot
                └─ short view install

query short view clone
    └─ scope/Qdrant/evidence/object I/O without manager/window mutation lock
       └─ bounded terminal output/window commit
```

No global lock nests another global lock. No redb writer, view-manager mutex or QueryWindow mutation borrow spans
filesystem, Qdrant, process or model work.

## Optional leaves

Encrypted storage (#598), watcher, regex, Qdrant Edge spike, PDF, MCP, model/rerank, overlay, managed Qdrant and
syntax/SCIP appear only after the baseline consumer exists. Each leaf reuses root/ObjectRef/preparation/evidence/
query boundaries and cannot create a second store, backend authority or session system.

## Removal direction

With each vertical cut remove displaced config registries, root leases/epochs, duplicate object writers,
preparation stores, proof/oracle publication paths, raw pin/handle catalogues, pairing/grant protocol, wave
features and exclusive fixtures. Recheck exact current callers at the deletion head; historical package maps are
evidence only.
