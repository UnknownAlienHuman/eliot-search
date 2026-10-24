# Daemon data-root composition

This directory currently combines root ownership with initialization, source registration/admission, control
journal restore, key selection, migration, ACL qualification, command parsing and many compatibility paths.
It is not the target application boundary.

Read the [architecture](../../../../docs/architecture/README.md),
[implementation guide](../../../../docs/implementation/README.md),
[root-owner audit](../../../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md),
[#266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
[#360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

## Verified scale

Excluding test-named files, `kernel.rs` plus `kernel/**` contains **443,814 bytes (~433.4 KiB) production
Rust**. Together with `search-runtime-owner` the root-ownership family exceeds **553 KiB before tests**.

Large code families include:

- command-shaped `NativeRootRequest`/work/digest accounting;
- fresh-root creation and eight-phase initialization journal;
- two owner locks plus alternating ACTIVE/DRAINING/RELEASED owner slots;
- successor/epoch/executable/process/request identity;
- direct-control/registry restore and source registration/admission;
- revision-DEK candidate/current selection;
- control-migration and one-shot/service wrappers;
- storage ACL observations and many exact compatibility records.

The useful authority is smaller: actual root/lock handles, current root-format validation, selected path/ACL
checks and ordinary ownership of opened current stores.

## Target daemon composition

```text
one parsed DaemonInvocation / DaemonSettings
→ inspect or explicit initialize
→ open current root + acquire DataRootGuard
→ open typed redb current state
→ open immutable object/source owners
→ open configured Qdrant runtime
→ construct publication/query/protocol handlers
→ serve
→ stop admission and close owners in reverse dependency order
→ drop DataRootGuard
```

The root guard is retained once by daemon composition. Source/query/publication/admin operations do not reacquire
or serialize it and do not receive an owner receipt.

## Root files and authority

The current-format root requires:

- one root-format manifest/profile published last at initialization;
- one process-lifetime exclusive lock file/handle;
- current redb/object directories/files owned by their packages;
- optional selected storage-profile metadata.

Do not preserve `.eliot-search-installation.v1`, owner slot A/B, ACTIVE/DRAINING/RELEASED process history,
owner epochs, successor records or dual legacy/sealed locks as current authority. Unsupported pre-release
roots are refused before writes and left intact.

## Separate responsibilities

Move/delete current code according to its actual owner:

| Current owner-composition area | Target |
|---|---|
| argument/command parsing in `NativeRootRequest` | one CLI/daemon invocation parser |
| initialization/fresh-root/journal | #360 staging + final manifest owner |
| lock/path/locality/no-reparse | small data-root platform owner |
| direct registry/control journal | typed current redb/source state |
| source registration/admission/sync | source inventory/reader/head owner |
| revision-DEK selection | accepted #598 immutable object/key profile only |
| control migrations | remove; unsupported old root remains intact |
| storage ACL | selected current storage/platform owner |
| one-shot/service command custody | current protocol/handler composition |
| owner succession/slots/lifecycle | delete; actual live lock owns exclusion |

No replacement manager coordinates all these domains.

## Open existing

```text
validate current invocation/config outside root owner
→ open/canonicalize selected local root without following replacement/reparse paths
→ retain root handle
→ open one current lock file and acquire exclusive lock
→ reread/verify root-format manifest through retained handles
→ open current redb/object/source/publication owners
```

Failure closes partial owners and drops the lock. A second owner receives `AlreadyOwned`. Do not infer ownership
from PID, executable hash, owner slot or process record.

## Initialize

Only explicit initialization writes an absent root. Build in unique owned staging and publish the root-format
manifest/namespace last. Normal open performs zero repair. Explicit resume/discard is limited to exact
uncommitted owned staging as defined by #360.

No initialization operation digest/original argv archive, second installation identity, owner epoch,
quarantine/finish receipt or generic recovery journal.

## Serve and shutdown

The running daemon owns one finite request set/semaphore, query windows and current owners. Shutdown:

1. stop listener/request admission;
2. cancel/drain finite requests and windows;
3. resolve/stop publication and Qdrant users;
4. close source/object/control owners;
5. drop the root guard.

No DRAINING/RELEASED record or dependency shutdown receipt is written. Real unresolved effects remain with
their concrete owner and are recovered on the next open from actual state.

## Removal boundary

After direct cutovers, delete:

- `native_request` digest/work-stage/command constructors;
- `initialization_journal`, old fresh-root lifecycle and owner slot codec/record/succession;
- dual lock/barrier compatibility;
- direct registry/source/DEK/migration command wrappers;
- generic owner capability metadata, release receipts and poisoning based on `effect_observed`;
- legacy service/one-shot custody wrappers replaced by the framed dispatcher;
- package-specific tests preserving these APIs.

Keep the small platform root guard, manifest staging/open, selected ACL/path checks and direct daemon composition.

#598 remains active private work. Consume its accepted current object/key seam only; do not inspect or create a
competing writer.
