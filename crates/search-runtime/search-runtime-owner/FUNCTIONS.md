# search-runtime-owner — source/caller audit and replacement map

## Gross production inventory

### Pure package

`search-runtime-owner/src` contains **123,087 bytes (~120.2 KiB)** before tests:

```text
admission.rs              7,617
error.rs                  5,378
identity.rs              10,456
lease.rs                 21,207
lib.rs                    1,960
state.rs                  7,857
supervisor.rs             8,981
transition.rs             4,044
transition/acquire.rs    32,247
transition/health.rs      5,366
transition/release.rs    17,974
```

`OwnerSupervisor::new` and the full prepare/complete/recover acquire API were found in package tests/internal
wrapper only. `classify_root_open` has one real daemon native-admission caller. Daemon code imports several
errors/enums, but that does not justify the pure lease/transition platform.

### Daemon owner composition

Excluding files named as tests, `bins/eliot-searchd/src/owner_composition/kernel*` contains **443,814 bytes
(~433.4 KiB)**. Large production files include:

```text
native_request.rs          43.5 KiB
initialization.rs          40.9 KiB
fresh_root.rs              30.0 KiB
read_child.rs              25.9 KiB
native_admission.rs        23.4 KiB
native_source.rs           23.6 KiB
native_dek_selection.rs    22.8 KiB
initialization_journal.rs  21.9 KiB
registration_mutation.rs   20.8 KiB
direct_registry_command.rs 19.8 KiB
direct_registry.rs         16.1 KiB
native_dek_command.rs      15.3 KiB
```

Combined pure package + daemon production source is **566,901 bytes (~553.6 KiB)** before tests.

This is gross inventory, not a promised net deletion. Real path/lock/staging/ACL/control/object effects remain.

## Current pure API and disposition

| Current API/module | Caller reality / disposition |
|---|---|
| `classify_root_open` | Real daemon native-admission caller. Keep a smaller classifier or inline beside explicit initialize/open. |
| `OwnerSupervisor` | Construction found in package tests; no normal native daemon owner uses it. Delete. |
| `prepare_acquire`, `complete_acquire`, recovery/release transitions | Used by `OwnerSupervisor`/tests, not the concrete native owner. Delete. |
| `OwnerIdentity`, `ProcessCreationIdentity`, `ExecutableIdentity`, `OwnerBinding` | Logical values fed into old records/leases. Actual authority is retained root/lock handles. Delete unless a small diagnostic field has a real consumer. |
| `OwnerGuard`, `LeaseWindow`, renewal/health/drain/release/dependency receipts | Logical state; not an OS lock or resource lifetime. Delete with callers. |
| generic mutation outcome classifier | Move/inline at real effect owners; different effects need different readback. |
| `OwnerError` | Retain only small current root/open/platform classes; delete historical lifecycle codes. |

## Current daemon path and defects

### Command-shaped root authority

`NativeRootRequest` stores:

- mode/root/deadline/cancellation;
- request ID and BLAKE3 digest of retained argv;
- copied native arguments;
- six cumulative limits: arguments, input bytes, encoded bytes, read bytes, native calls and opens;
- named `NativeWork` stage arrivals and terminal refusal state.

It also contains constructors/parsers for root registration, Source-v2 admission, source sync, DEK prepare/
selection, service, control migrations, one-shot reads and other commands.

The invocation owner should parse a command once. Root ownership should receive only:

```text
root path
inspect | initialize | open-mutating
absolute deadline/cancellation
small path/read limits
```

Do not bind the entire application command/argv digest to a root lock.

### Existing open

`native_admission.rs` currently:

1. checks Windows-only mode and canonical local path;
2. opens/retains the root directory;
3. checks many legacy/direct barriers;
4. opens both `.eliot-search-owner.lock` and `.eliot-search-sealed-owner.lock` and calls `try_lock`;
5. parses `.eliot-search-installation.v1`;
6. reads alternating owner slots and expects `RELEASED`;
7. opens pins, storage ACL observations and optionally direct control;
8. plans/publishes a successor ACTIVE owner record;
9. carries selected key binding, request/work meter and effect-observed poisoning in one capability.

The two actual locked file handles provide exclusion. Current-format open does not need legacy/sealed dual
locks plus alternating lifecycle records. Storage/control/key/source owners open after the guard and manage
their own state.

### Durable owner format

`spec.rs` defines:

```text
.eliot-search-installation.v1
.eliot-search-owner-state-a.v1
.eliot-search-owner-state-b.v1
ACTIVE | DRAINING | RELEASED
owner epoch / generation / request digest / executable digest / drain reason
```

A crash releases the OS lock but leaves stale lifecycle records. Normal restart then needs succession/recovery
to reason about data which is not exclusion authority. Delete this durable process-history model. One current
root-format manifest describes disk format/profile; process ownership is the live lock handle.

### Initialization

Current initialization/fresh-root/journal code creates a large operation/lifecycle protocol. Replace it with
#360 unique staging and final root manifest publication last. Normal open never repairs staging. Explicit
resume/discard is limited to exact owned uncommitted staging.

### Shutdown

Current code models dependency shutdown receipts, drain tokens and owner release. Replace with direct daemon
order and RAII/drop. A released lock has no persistent receipt. A real unresolved publication/object/control
effect is resolved by that owner before shutdown completes.

## Target concrete owner

Suggested behavior, not mandatory names:

```text
inspect_root(path) -> RootInspection
initialize_root(path, settings, context) -> CurrentRootMetadata
open_current_root(path, context) -> OpenedDataRoot

OpenedDataRoot {
  canonical root handle
  exclusive lock handle
  current root-format metadata
}
```

`OpenedDataRoot` is held by daemon composition. Opening current stores is a separate step so failures close
partial owners and release the lock through ordinary ownership.

### Path and lock properties

Keep the selected Windows/native properties which are real:

- canonical local path; reject UNC/device/reparse/unsupported location according to current profile;
- no-follow/open-existing semantics for committed root/manifest/lock;
- one create-on-initialize/current-format lock file;
- exclusive `try_lock` held for process lifetime;
- reread manifest/root identity through retained handles after lock to prevent path replacement;
- optional actual ACL/user validation owned by the selected storage profile.

Do not add PID liveness, process executable hashing, owner epochs or a second lock as fallback authority.

### Root format metadata

Keep one small current-format manifest/profile containing only disk compatibility/identity needed to open the
stores, for example:

```text
format version
root ID
storage profile
control/object schema version
```

This is immutable/current root format, not process owner state. Unsupported versions are refused before
writes and left intact.

## Removal order

1. Implement #360 staged manifest-last initialization and side-effect-free inspection.
2. Open current root with one real guard and direct current-format metadata.
3. Open typed redb/object/source/publication owners under the held guard.
4. Change service/source/query/admin callers to use daemon-owned owners rather than command-specific root
   capabilities.
5. Delete root parsing from `NativeRootRequest`, named stage/work meters and command-specific custody.
6. Delete owner slots/succession/lifecycle/dual locks/initialization journal.
7. Delete pure supervisor/lease/transition package APIs and move/inline the small retained classifier/errors.
8. Remove old `search-runtime-owner` dependency if no independent package remains.

## Failure semantics

- invalid/unsupported/corrupt/foreign path before lock: known refusal, no writes;
- lock held: `AlreadyOwned`;
- known initialization/open transaction failure before final manifest: known failure/staging remains bounded;
- final manifest publication acknowledgement uncertain: explicit initialization inspection decides committed vs
  incomplete; no generic owner operation replay;
- current store open failure: close opened stores/drop lock, leave root unchanged;
- shutdown component failure: report the concrete component; do not write DRAINING/RELEASED as compensation;
- process crash: OS releases lock; next open validates current root/stores directly.

## Focused checks

- inspect absent/current/unsupported/corrupt/incomplete does zero writes;
- initialize crash points yield either committed current root or recognized staging;
- one lock excludes a second process and releases on crash/drop;
- path replacement/reparse/foreign volume refused according to profile;
- stale historical owner files are not consulted by current open;
- current stores open after guard and close before release;
- shutdown/cancellation does not turn known local abort into unknown external effect;
- no source/DEK/registry/migration command parsing remains in root ownership.
