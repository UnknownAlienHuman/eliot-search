# Working on search-ports

This package is a removal target. Read [README.md](README.md), the
[architecture](../../docs/architecture/README.md) and
[implementation guide](../../docs/implementation/README.md).

Do not add or harden a global port, associated-type family, generic receipt, mutation identity, capability,
conformance descriptor or fake implementation. Do not preserve old trait names to make obsolete consumers
compile.

When changing a consumer:

1. connect it directly to the concrete current owner;
2. pass current domain values and one absolute operation/request context where needed;
3. return actual readback/state or a domain error;
4. move any genuinely reusable small type/helper to its natural owner;
5. delete the old port import, adapter, conformance entry and fixture in the same cutover.

A new trait is allowed only beside a real consumer which currently needs multiple implementations. Keep it
small, concrete and local. Test fakes remain beside that consumer and do not become a product-wide registry.

Do not create `search-ports-v2`, forwarding crates or a service locator. The end state deletes this package,
its workspace dependencies, historical port catalogues and generated coverage consumers.

After production changes, run minimal Rust 1.98 compiler/Clippy for the changed owner and immediate consumers.
Port-catalog counts, fake failure matrices and source-shape checks are not acceptance gates.
