# search-handles

Current handle lookup and retained-source expansion code; the baseline result need not carry a
separate handle lifecycle.

## Current source

[lib.rs](src/lib.rs) implements `HandleStore`, mint/resolve, expiry/invalidation, `revalidate` and
`expand`; [invalidation.rs](src/invalidation.rs) adds cursor-based security/binding invalidation.
Both handle classes live in memory. `mint_durable_source` does not persist a record or acquire a
retention lease.

The daemon constructs this store in
[native binding composition](../../../bins/eliot-searchd/src/access_composition/native_binding/process.rs)
and registers lifecycle consumers. Public DIRECT search instead mints and expands through its separate
[ResultHandleCatalog](../../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs).
These are duplicate owners, not a completed shared query/expansion path.

## Implementation direction

Use source/revision/range references for trusted-local results and reopen them through the real retained
reader. If a selected client needs opaque locators, resolve them to items in the
[same temporary query window](../search-continuation/README.md#one-temporary-owner), with its scope and
expiry. Do not add another registry or preserve both handle catalogs.

Keep actual source/range/read limits and selected client restrictions. Change the public result,
expansion caller and invalidation consumers with the owner. [FUNCTIONS.md](FUNCTIONS.md) records current
defects; the [implementation guide](../../../docs/implementation/README.md#turn-a-point-into-evidence)
owns retained-byte validation.
