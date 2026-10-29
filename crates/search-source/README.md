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

`Plain` is the baseline current-format profile and has no secret dependency. It must implement the complete
immutable-object contract now.

An encrypted profile is optional. #598 owns only that profile and may be integrated after an accepted public
handoff. It does not block plain retention, preparation, Qdrant publication, evidence, Exact or CLI work. Do not
inspect or modify #598's private branch/worktrees.

Both profiles, when present, produce the same Eliot-owned `ObjectRef` semantics. Do not create parallel source
registries, preparation stores or query paths.

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
