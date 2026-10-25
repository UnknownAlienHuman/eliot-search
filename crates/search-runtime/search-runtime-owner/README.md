# search-runtime-owner — one real root guard, not a lifecycle protocol

The product needs one small boundary: inspect or initialize one current-format data root, open it through retained
handles, and hold one actual exclusive process-lifetime guard while all stores are open.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source map](FUNCTIONS.md),
[root issue #266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
[initialization issue #360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

## Revision-specific evidence

At the original inspected public-main baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, this package plus daemon
owner-composition code contained about 553.6 KiB of production Rust before tests. The audit found a large
lease/identity/epoch/receipt/initialization-journal system around real root and lock handles. Those measurements
and caller findings explain the selected cutover; they are not a current branch census or a promised deletion
amount.

Before deleting, merging or inlining this package, repeat an exact search at the implementation head across
workspace/package manifests, Rust imports and constructors, non-Rust tooling, feature/target wiring, tests and
workflows. Move every current caller in the same cut. Do not harden an obsolete API merely because it remains
reachable from tests or compatibility wiring.

## Target ownership

```text
DataRootGuard {
  retained canonical/open root handle
  one actual exclusive lock handle
  current root-format identity/profile
}
```

The selected platform implementation may retain private handles needed to prevent replacement or validate
locality, reparse-point and access properties. These are implementation details, not public owner records.

The daemon holds the guard from successful current-root open until source, object, control, publication, query
and protocol owners are closed. Internal operations borrow those owners; they do not renew, serialize or prove
the guard.

## Operations

### Inspect existing

Inspection is side-effect free and reports only useful states: absent, current, incomplete initialization,
unsupported pre-release, corrupt, busy/owned, or foreign/unsafe path. It creates no directory, lock, journal,
owner record or repair state.

### Initialize new

Initialization is an explicit command:

```text
validate selected local target and absence/foreign layout
→ acquire actual initialization exclusion
→ build current layout in unique owned staging
→ flush, close and verify stores
→ publish one current root-format manifest/namespace last and without clobber
→ verify final layout
→ remove bounded staging debris
```

A crash before final publication leaves recognizable incomplete staging. Explicit resume may finish only a
fully verified owned staging publication; explicit discard may remove only exact uncommitted owned staging.
Normal open does not repair or migrate.

### Open existing

```text
parse invocation/settings before root effects
→ validate/open current root without following replacement/reparse paths
→ acquire one exclusive process-lifetime lock
→ reread and verify current root metadata through retained handles
→ open concrete redb/object/source/publication owners
→ retain DataRootGuard in daemon composition
```

No owner-slot succession, PID/executable digest, request digest, owner epoch, lease renewal or recovery receipt
is authority. Process death releases the actual lock; the next open validates current state directly.

### Shutdown

Stop admission, cancel or drain finite in-process work, drop query windows and view guards, resolve concrete
publication effects, close control/source/object owners, then drop `DataRootGuard`. Use ordinary ownership and
one bounded shutdown context. Do not write DRAINING/RELEASED records as compensation.

## Removal boundary

After current callers move, remove the generic supervisor/acquire/recover/drain/release state machine, logical
owner/process identities, lease windows, alternating owner slots, dual legacy/sealed locks, request/argv digests,
named work-stage protocol, command-specific source/key/registry/migration custody and fixtures that only preserve
those APIs.

Keep only actual path/locality/no-reparse checks, retained root and lock handles, one current-format classifier,
explicit staging initialization, bounded context, content-free failures and direct shutdown ordering. If the
remaining code is small and has no independent dependency boundary, inline it into daemon root composition and
delete the package.

Stopped-root local-object collection and archive/import are separate maintenance operations documented in
[`search-retention/FUNCTIONS.md`](../search-retention/FUNCTIONS.md). They acquire the same real guard after the
normal daemon is stopped; they are not responsibilities of the open-root owner.

## Completion

A fresh root initializes crash-safely; inspection performs no writes; exactly one process owns an open root
through one actual guard; stores open after the guard and close before it; unsupported roots remain untouched;
and no lease/receipt/succession platform remains. After code, run minimal Rust 1.98 check/Clippy for the changed
owner and actual daemon/store consumers, then focused inspect/initialize/lock/restart/path-replacement/shutdown
cases. Historical lifecycle matrices disappear with their APIs.
