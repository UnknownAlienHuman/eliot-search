# search-runtime-owner — source map

The [implementation guide](../../../docs/implementation/README.md) owns the application path. This file
describes implemented code and what remains useful after simplification.

## Implemented code

| Source/API | Actual behavior |
|---|---|
| [admission.rs](src/admission.rs): `classify_root_open` | Purely maps one `DataRootOpenMode` and observed `RootOpenState` to read, initialize-proof, live-owner, exact-recovery or refusal. Daemon native admission calls it. It mints no guard and performs no I/O. |
| [identity.rs](src/identity.rs) | Value types for root, process, executable and owner identity. Their presence does not establish a native observation. |
| [lease.rs](src/lease.rs) | `OwnerGuard`, drain/release tokens, health values and dependency shutdown receipts. These are logical records, not an OS lock or daemon shutdown. |
| [state.rs](src/state.rs), [transition.rs](src/transition.rs) | Pure acquire/recovery/drain/release state transitions over supplied observations/effect outcomes. |
| [supervisor.rs](src/supervisor.rs) | In-memory wrapper around the transition model. Search found package-internal/test use, not the normal native owner implementation. |
| daemon [native admission](../../../bins/eliot-searchd/src/owner_composition/kernel/native_admission.rs) and owner composition | Own actual root files, native exclusion, initialization/current records and operation custody. These effects are the product boundary. |

The current daemon imports several enums/errors from this package, but that does not require preserving
every logical lease, receipt and transition family. Change the pure API and its concrete callers together.

## Target operations

### Inspect

Classify an existing root without creating directories, namespace files, logs, owner records or migration
artifacts. Return absent, current-format, unsupported, corrupt, foreign or unresolved. Inspection is
side-effect free.

### Initialize

Run only from an explicit initialize command. Require the selected local root, exclusion/containment and
empty-layout proof before creating the current format. Same-operation recovery is needed only for an
actual initialization effect whose outcome may be unknown.

### Open existing

Acquire one real process-lifetime exclusive guard, then open the existing control/object owners. The guard
is retained by daemon composition and borrowed implicitly through those owners. Ordinary source, query and
publication calls do not renew it or carry serialized owner receipts.

### Shutdown

Stop admission, cancel/drain active work, close query windows/backend writers/control and object writers,
then release the native guard. Use ordinary ownership/drop where sufficient. Keep explicit recovery only
when release itself has a real uncertain durable/native effect.

## Removal boundary

Delete or inline after consumer migration:

- abstract renewable lease windows and health histories;
- dependency receipt catalogues used only to authorize release;
- generic acquire/release operation archives;
- mode-transition framework beyond explicit restart-required configuration;
- duplicate owner supervisors and forwarding wrappers around daemon-owned state.

Do not replace them with a new root manager. Keep the small open classifier or move it beside its sole
consumer if that leaves a clearer boundary.

## Behavior to verify

- inspection of an absent/unsupported root performs zero writes;
- initialize and open-existing are not interchangeable;
- exactly one daemon owns a root at a time;
- a second process cannot infer ownership from PID/path/record alone;
- a failed local transaction is not reported as an unknown external effect;
- actual unresolved initialization/control/object/publication work is recovered by its owner;
- shutdown releases the native guard after its concrete dependents close.
