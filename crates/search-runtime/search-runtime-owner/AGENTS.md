# Working on search-runtime-owner

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), package [README](README.md),
[source audit](FUNCTIONS.md), root issue [#266](https://github.com/UnknownAlienHuman/eliot-search/issues/266) and
initialization issue [#360](https://github.com/UnknownAlienHuman/eliot-search/issues/360).

The product needs one actual process-lifetime root guard, not a lease/receipt/state-machine platform.

Keep only real properties:

- side-effect-free root inspection;
- explicit manifest-last initialization;
- selected path/locality/no-reparse/current-format checks;
- one retained root handle and one exclusive lock handle;
- small current root-format metadata;
- close real stores before dropping the guard;
- content-free failures and one absolute context.

Delete or inline:

- `OwnerSupervisor`, owner snapshots and generic acquire/recover/release transitions;
- owner/process/executable identities not consumed by the live lock;
- lease windows, renewal, health histories, drain/release/dependency receipts;
- alternating owner slots, owner epochs/generations/succession and dual legacy/sealed locks;
- complete argv/request digest, named work stages and command-specific root constructors;
- source/DEK/registry/migration custody inside root ownership;
- initialization journal/state taxonomy replaced by #360 staging;
- fixtures preserving those APIs.

The daemon invocation owner parses commands. The source, storage, control, publication and key owners manage
their own state after the root guard. Do not create `runtime-owner-v2`, forwarding wrappers or another owner
record.

After code, run minimal Rust 1.98 compiler/Clippy for the root owner and actual daemon/store consumers, plus
focused inspect/initialize/lock/restart/path-replacement/shutdown cases. Historical lifecycle/receipt matrices
are removed with their APIs.
