# Working on search-runtime-owner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), package [README](README.md),
[source audit](FUNCTIONS.md), root issue [#266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
initialization issue [#360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

The product needs one actual process-lifetime root guard, not a lease/receipt/state-machine platform.
Historical byte counts and no-caller findings in this package are revision-specific evidence. Immediately before
moving or deleting code, recheck current workspace/package manifests, Rust and non-Rust callers, constructors,
features, targets, tests and workflows. Change all actual callers in the same cut.

Keep only real properties:

- side-effect-free root inspection;
- explicit manifest-last staging initialization;
- selected path/locality/no-reparse/current-format checks;
- one retained root handle and one exclusive lock handle;
- small current root-format metadata;
- concrete stores opened after the guard and closed before it;
- content-free failures and one absolute bounded context.

Remove after caller cutover:

- `OwnerSupervisor`, owner snapshots and generic acquire/recover/release transitions;
- logical owner/process/executable identities not consumed by the live lock;
- lease windows, renewal, health histories, drain/release/dependency receipts;
- alternating owner slots, epochs/generations/succession and dual legacy/sealed locks;
- complete argv/request digests, named work stages and command-specific root constructors;
- source/key/registry/migration custody inside root ownership;
- initialization journal/state taxonomy replaced by #360 staging;
- fixtures preserving retired APIs.

The invocation owner parses commands. Source, storage, control, publication and key owners manage their own
state under the held root guard. Stopped-root local-object maintenance/archive/import is a separate explicit
operation, not a runtime-owner subsystem. Do not create `runtime-owner-v2`, forwarding wrappers or another
durable owner record.

After code, run minimal Rust 1.98 check/Clippy for the changed owner and actual daemon/store consumers. Focused
runtime cases are inspect, initialize, exclusive lock, restart, path replacement and shutdown; historical
lifecycle/receipt matrices are deleted with their APIs.
