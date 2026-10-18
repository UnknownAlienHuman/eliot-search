# search-runtime-owner

Small pure policy helpers for opening and exclusively owning one local data root.
The daemon performs the filesystem/process effects; this package must not become a second runtime or
recovery operating system.

Read the [architecture](../../../docs/architecture/README.md) and
[implementation guide](../../../docs/implementation/README.md). [FUNCTIONS.md](FUNCTIONS.md) records
the current source and the cutover.

## Current source

- [admission.rs](src/admission.rs) defines `InitializeNew`, `InspectExistingReadOnly`,
  `OpenExistingMutating` and named recovery. `classify_root_open` is used by native daemon admission.
- `identity`, `lease`, `state`, `transition` and `supervisor` model a much broader owner lifecycle from
  caller-supplied observations and effect results. The package itself performs no lock, process or durable
  record I/O.
- Daemon owner composition already owns the actual native files, exclusion handles, current owner record,
  initialization journal and shutdown order. A second abstract owner history does not make those effects
  safer or more connected.

## Required result

Keep the root boundary small:

```text
explicit initialize-new command
  → prove selected empty local root
  → create one current-format layout

normal daemon open
  → inspect existing layout without writes
  → acquire one OS/process-lifetime exclusive guard
  → open redb/object writers
  → release them in ordinary shutdown order
```

Inspection and ordinary open never initialize, migrate or repair missing state. Unsupported old formats
fail before writes and remain intact. A root guard is process-local authority to use already-open owners;
it is not a serializable lease or a credential for every internal operation.

Keep real second-owner denial, exact local-root identity, no-follow/native checks and the distinction
between known failure and a genuinely unresolved external effect. Recover unresolved control/object/
publication effects in their actual owners. Do not build a generic acquisition/release journal to wrap
all of them.

## Remove with callers

Retire renewable lease windows, generic owner epochs detached from the actual guard, standalone/managed
mode histories, dependency-shutdown receipt graphs, permanent acquire/release operation archives and
supervisor/facade layers that have no remaining concrete daemon consumer. Preserve useful open-mode
classification and native exclusion mechanics while changing their direct callers.

Check explicit initialize versus existing open, two-process exclusion, read-only inspection with zero
writes, unsupported/corrupt roots, crash while a real mutation is pending and orderly guard release.
No package shape, receipt graph or owner-state golden substitutes for opening the actual application.
