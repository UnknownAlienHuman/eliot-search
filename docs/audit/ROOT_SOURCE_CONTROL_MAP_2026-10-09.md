# Root/source/control authority map — 2026-10-09

**Audited main:** `2ae7ecbdf7dced3cc951e5e162dbba10def03dbc`  
**Coordinator:** #97  
**Product ready:** no

## Current decision

```text
#237
→ #266 one data-root owner/open authority
→ #267 one active RootRecord/live safe-read authority

#237 + #266
→ #268 complete legacy inventory/staged redb candidate
→ #269 one atomic redb current authority

#237 + #241 + #267 + #269
→ #270 pre-admission + source identity index + bounded staging + source commit

#239 + #267 + #269 + #270
→ #271 durable root/workspace reconcile + immutable SourceView

#269 + #271
→ #272 immutable corpus/portfolio history + guarded aliases + scope compiler
```

## Retired packet branches

| Closed PR | Historical role | Executable successor |
|---:|---|---|
| #100 | native identity tracking | #267/#215 |
| #104 | safe-reader authority tracking | #267 |
| #105 | root owner tracking | #266 |
| #106 | redb tracking | #268/#269/#261 |
| #107 | migration tracking | #268 |
| #108 | cutover tracking | #269 |
| #110 | ingestion program | #241/#267/#269/#270/#271 |
| #128 | currentness program | #239/#267/#269/#270/#271/#272 |

Source already landed directly on main remains useful; the one-file PR branches are not implementation bases.

## Verified current-main defects

### F62 — read can initialize durable state

`DirectStore::open` creates control/revision directories, namespace and source log. A read-only operation that uses it is not read-only.

### F63 — immediate parent is treated as admitted root

`read_file_snapshot` passes `absolute.parent()` to the safe-reader adapter. This proves containment only under an arbitrary parent, not an active registered RootRecord.

### F64 — security/location authority is fabricated

The safe-reader adapter uses constant barrier revision 1, reports `Permitted` unconditionally and can classify a filesystem source `LocalFixed` without native volume/share proof.

### F65 — legacy and redb remain dual control authorities

Migration inspection and staged redb code exist, but normal serving still reaches legacy state until one exact cutover and routing decision lands.

### F66 — ingestion reads before policy and scales as a cross product

Current composition can read full bodies before admission, scan/clone the prior identity corpus per file and retain too much batch state.

### F67 — watcher/per-root completion is not coherent currentness

Watcher silence and sequential root success do not prove one workspace state. Every root and watcher generation must be reobserved after the final effect.

### F68 — mutable current-only portfolio storage breaks historical meaning

A later portfolio revision can replace the record addressed by the same ID. Exact old scope revisions therefore require append-only `(id, revision)` records and separate guarded pointers.

## Authority boundaries

### #266 owns

- inspect/mutate/initialize/recovery open modes;
- uniform quarantine/outcome-unknown behavior;
- one root owner across control/source/Qdrant/provider children;
- reverse shutdown and abandoned-owner recovery.

### #267 owns

- RootRecord-relative locator/read binding;
- native root/file/location observation;
- live security/access/purge barrier revalidation;
- final-handle race-safe bytes.

### #268/#269 own

- complete legacy source/target accounting;
- staged redb mapping/readback;
- one atomic current-authority cutover;
- legacy bytes as read-only evidence, no fallback or dual write.

### #270 owns

- content-free pre-admission before body read;
- one verified source identity index per SourceView;
- bounded immutable staging/backpressure;
- exact redb source/head/reference operation.

### #271 owns

- watcher hints/gaps only;
- durable per-root/workspace reconcile phases;
- final all-root reobservation;
- immutable complete/partial SourceView revisions.

### #272 owns

- immutable SourceView/corpus/portfolio revision storage;
- separate guarded aliases/defaults;
- exact authorized population shared by exact search, retrieval, IDF, count, validation and ranking.

## Anti-Frankenstein constraints

- no create-or-open normal product API;
- no second root/source/control catalog;
- no path/parent/PID/quiet watcher as authority;
- no caller `fresh/complete/allowed` proof;
- no dual legacy/redb write or fallback;
- no body read before required pre-admission;
- no per-item corpus clone/scan;
- no current-path rescan to guess an unknown durable effect;
- no mutable alias containing the only revision bytes;
- no source-current flag automatically enabling indexed readiness.

## Completion boundary

The root/source/control program is complete only when one owner opens the root, one redb store is current authority, all bytes are read under an active RootRecord/live security, every source effect is durable/recoverable, one immutable coherent SourceView is published after final all-root revalidation and all corpus/portfolio history remains exactly resolvable.
