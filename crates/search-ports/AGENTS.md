# Working on search-ports

This package is a deletion target. Read [README.md](README.md), the exact
[source/caller census](FUNCTIONS.md), the [architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md).

Do not add, harden or version a global port, associated-type family, operation context, generic receipt,
mutation identity, capability, conformance descriptor or fake implementation. Do not preserve a trait name
merely to make an obsolete consumer compile.

When changing a consumer:

1. identify the concrete current owner and actual effect;
2. call its direct API with current values;
3. carry an absolute deadline/cancellation and real counters only where the operation needs them;
4. resolve uncertain effects through authoritative state/readback;
5. move a genuinely reusable small helper to its natural owner;
6. delete the old import, adapter, conformance entry, generated map and exclusive fixture in the same slice.

A new trait is allowed only beside one real consumer that simultaneously needs multiple implementations. It
must be small, concrete and local. Test fakes remain beside that consumer and do not become a product-wide
registry.

The end state removes `crates/search-ports`, its workspace dependency, P00 port catalogues and generated
coverage consumers. Do not create `search-ports-v2`, forwarding crates, a service locator or an effect bus.

After production changes, run minimal Rust 1.98 check and strict Clippy for the changed owner and immediate
real consumers. Port counts, fake matrices and source-shape tests are not acceptance gates.
