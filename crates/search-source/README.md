# Sources — one retained-body path

A selected source becomes immutable retained bytes, one current source head, one preparation outcome and later
published evidence. The [implementation spine](../../docs/implementation/README.md) defines delivery order.

## Baseline path

```text
explicit selected file
→ pre-read policy
→ one stable final-handle/Git-object read
→ post-read classification
→ plain immutable put_object
→ one typed SourceRecord compare/set
→ one preparation result
→ later PublishedView
```

The first walking skeleton supports one explicit local file. Root inventory and periodic reconciliation follow
after the end-to-end Search path works.

## Storage profiles

There is one plain immutable-object format under [local scope](../../docs/architecture/LOCAL_SCOPE.md).
No key, cipher, security-domain or storage-mode field is needed. Source/preparation/evidence use the same ObjectRef.
Preserve unpublished #598 work without waiting for or importing it. Do not add parallel registries or query paths.

## Identity and current state

- a path locates a source;
- `SourceId` is persisted current identity;
- a non-reset row/registration revision rejects stale work across A→B→A and remove/re-add;
- content identity names a revision, never the logical source;
- an optional exact physical key may support rename matching;
- Git evidence binds selected tree/path/object identity and retained bytes.

Current source state may be newer than the served `PublishedView`. Old queries remain coherent through their
pinned view and exact ObjectRefs, not path history or registry replay.

## Ownership

| Responsibility | Owner |
|---|---|
| root containment and stable read | root/safe-reader composition |
| selected-source policy | one small admission evaluator |
| immutable bytes | revision-store object owner |
| roots/sources/current heads | typed redb/source methods |
| changed/missing/incomplete observations | reconcile owner |
| source transformation | preparation owner |
| served evidence | PublishedView + immutable reader |

Delete receipt, provenance, history, portfolio, restore and replay platforms after their actual callers move.
Keep only concrete current rows and behavior used by the connected path.
