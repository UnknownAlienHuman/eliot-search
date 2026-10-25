# Immutable object store — plain baseline, optional encrypted profile

This owner persists source bodies and prepared artifacts. It decides object identity, publication, verification,
range reads and exact deletion mechanics. Source membership, preparation currentness, publication reachability and
query scope belong elsewhere.

## Baseline is not blocked by #598

Implement the `Plain` current-format profile first. It has no key, secret catalogue or encryption lifecycle.
#598 may later provide an encrypted profile after an accepted public handoff. Integration must use the same
Eliot-owned object boundary and must not create a second source/preparation/evidence path.

If #598 overlaps existing files, coordinate the smallest public type seam with its manager. Do not wait for the
whole encrypted programme to implement the plain profile, and do not inspect private worktrees.

## Required boundary

Exact names may differ:

```text
put_object(kind, Plain, owned_bytes, RequestContext) -> ObjectRef
open_object(ObjectRef, RequestContext) -> VerifiedObject
read_ranges(ObjectRef, ranges, RequestContext) -> VerifiedRanges
list_objects(cursor, limit) -> bounded identities
remove_exact(ObjectRef) -> Absent | Removed | Unresolved
```

`ObjectRef` binds:

```text
current object-format version
storage profile
kind
content address
exact length
```

It carries no source membership, current-head, access or query authority.

## Plain publication

```text
validate final namespace/address and byte limit
→ create unpredictable create_new stage in final directory
→ write exact bytes while computing/verifying content identity
→ file sync according to the supported durability profile
→ publish final address no-clobber
→ when final already exists, open and verify it exactly
→ verify final object once
→ remove this writer's stage
→ return ObjectRef and optionally already-owned verified bytes
```

Two writers of the same bytes converge. Different bytes at one address are an immutable conflict. A known failure
before publication removes the stage when possible. A possible publication is resolved by exact final-address
readback. Leftover stages/unreferenced finals are stopped-root maintenance input, not operation history.

Do not claim Windows sudden-power-loss durability from final readback or a no-op directory sync. Select and
execute one qualified same-volume no-clobber primitive or expose the weaker durability profile explicitly.

## Reads

- never reacquire a mutable live source path;
- validate format/profile/kind/length before returning bytes;
- use the request's original deadline, cancellation and byte counters;
- reject declared oversize before allocation/I/O;
- share one required integrity pass across ranges from the same object;
- return exact coordinates and scanned-byte accounting;
- corruption, wrong profile and short/long content are hard read failures.

A digest returned beside arbitrary bytes is not verification.

## Reachability and deletion

The object store enumerates and removes exact objects. It does not decide reachability. Stopped-root maintenance
marks from current source/preparation state, published/pending state and retained archives, then sweeps exact
unreferenced objects. Failed Windows deletion remains visible for a later run.

No online object GC, tombstone database, purge proof or completed-operation archive is part of the baseline.

## Encrypted profile handoff

After #598 publishes an accepted implementation, retain only:

- the concrete encrypted `put/open/read` path;
- exact profile metadata needed by `ObjectRef`;
- its real OS-protected key owner and authenticated envelope checks.

Plain and encrypted profiles may share small object format/address helpers. Do not restore a universal secret
catalogue, provider registry, rotation/migration platform or legacy six-domain protocol.

## Remove with callers

Delete duplicate materializer writer, contextual receipt/stage APIs, in-memory object lifecycle histories,
legacy inventory/migration codecs and old DIRECT formats after exact current callers move.

## Focused checks

Plain profile first: zero/nonzero bytes, concurrent identical writers, final conflict, stage cleanup, restart
reopen, corruption, range bounds, cancellation and truthful durability status. Encrypted cases follow only after
the accepted #598 handoff.
