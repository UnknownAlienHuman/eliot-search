# search-runtime-owner — one real root guard, not a lifecycle protocol

The product needs one small boundary: inspect/initialize/open one current-format data root and hold one actual
process-lifetime exclusion guard while its stores are open.

Public main instead contains more than **553 KiB of production Rust before tests** across this package and the
daemon owner composition:

| Area | Gross production source |
|---|---:|
| `search-runtime-owner/src` | 123,087 bytes (~120.2 KiB) |
| daemon `owner_composition/kernel*` | 443,814 bytes (~433.4 KiB) |
| combined | 566,901 bytes (~553.6 KiB) |

Most of that code models leases, owner identities/epochs, request digests, alternating durable owner slots,
drain/release receipts, initialization journals, command-specific admission and migration/key/source wrappers.
The useful authority is the actual retained root/lock handles in daemon composition.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source map](FUNCTIONS.md),
[root issue #266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
[initialization issue #360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

## Target ownership

```text
DataRootGuard {
  canonical/open root handle
  one actual exclusive lock handle
  current root-format identity/profile
}
```

The exact platform implementation may hold additional private handles needed to prevent replacement or
validate locality/reparse/ACL properties. They remain implementation details, not public owner records.

The daemon owns the guard from successful current-root open until every redb/object/publication/query owner is
closed. Internal operations borrow those owners; they do not renew, serialize or prove the guard.

## Operations

### Inspect existing

Side-effect free. Return only useful states:

```text
Absent
Current(CurrentRootMetadata)
IncompleteInitialization
UnsupportedPreRelease
Corrupt
Busy/Owned            // when exclusive open is requested and lock is held
Foreign/UnsafePath
```

Inspection does not create directories, lock files, installation records, logs, journals or repair state.

### Initialize new

Explicit command only. Follow #360:

```text
validate selected local target and absence/foreign layout
→ acquire the actual initialization exclusion
→ build current layout in unique owned staging
→ flush/close/verify stores
→ publish one current root-format manifest/namespace last, no-clobber
→ verify final current layout
→ remove staging debris
```

A crash before the final publication leaves recognizable incomplete staging, not an ACTIVE/DRAINING/RELEASED
owner state. Explicit resume may only finish a fully verified owned staging publication; explicit discard may
only remove exact uncommitted owned staging.

### Open existing

```text
parse invocation/config before root effects
→ validate/open current root without following replacement/reparse paths
→ acquire one exclusive process-lifetime lock
→ reread/verify current root-format manifest through retained handles
→ open current redb/object owners
→ retain DataRootGuard in daemon composition
```

No owner-slot succession, PID/executable digest, request ID/digest, owner epoch or recovery receipt is needed.
An OS lock released by process death is not made safer by a stale durable ACTIVE record.

### Shutdown

```text
stop accepting requests
→ cancel/drain finite in-process work
→ drop query windows/view guards
→ finish/resolve publication and close Qdrant users
→ close redb/object/source owners
→ drop DataRootGuard / lock handle
```

Use ordinary ownership/drop and one process request count/semaphore. No dependency receipt catalogue,
DRAINING record, release permit or shutdown receipt.

## Current source defects to remove

- `OwnerSupervisor` and `prepare_acquire` family have package-internal/test callers, not the native product
  owner.
- `NativeRootRequest` copies/hashes the entire invocation, carries six work-meter dimensions, named stage
  arrivals and constructors for source, DEK, migration, registry, service and diagnostic commands. The root
  owner should receive a root path/open mode/context, not parse every command.
- existing open acquires both `.eliot-search-owner.lock` and `.eliot-search-sealed-owner.lock`; the current
  format needs one lock authority, not legacy dual ownership.
- durable `.eliot-search-owner-state-a.v1` / `-b.v1` records alternate ACTIVE/DRAINING/RELEASED generations,
  request digests, executable identity and owner epochs while the real lock already owns exclusion.
- current open binds storage ACL, selected key binding, control journal, registry/source/DEK/migration logic
  into one capability. Those belong to their concrete owners after the root guard.
- cancellation after any `effect_observed` is broadly reclassified as owner acquire/release unknown even when
  a local abort or read has a known result.

## Removal boundary

Delete after direct daemon cutover:

- `OwnerSupervisor`, `OwnerSnapshot`, generic acquire/recover/drain/release transition framework;
- `OwnerIdentity`, executable/process creation identity and owner binding records not needed by the actual lock;
- `LeaseWindow`, renewal, health history, drain/release tokens and dependency/shutdown receipts;
- alternating durable owner slots, lifecycle/generation/epoch/succession records;
- dual legacy/sealed lock model;
- raw invocation/request digest and cumulative named `NativeWork` protocol;
- generic operation IDs, recovery evidence and mutation outcome wrappers;
- owner-composition DEK/source/registry/migration command parsers and forwarding custody;
- initialization journal/state taxonomy replaced by manifest-last staging;
- package/daemon fixtures protecting those APIs.

Keep only actual path/locality/no-reparse checks, root/lock handles, one current-format manifest classifier,
bounded context, content-free failures and explicit initialization/open/shutdown composition.

## Completion

A fresh root can be explicitly initialized crash-safely; normal open performs no repair or migration; exactly
one daemon owns the root through one actual guard; restart ignores stale process metadata because none is
authority; current stores open after the guard and close before it; unsupported/pre-release roots remain
untouched; and no owner lease/receipt/succession system remains.
