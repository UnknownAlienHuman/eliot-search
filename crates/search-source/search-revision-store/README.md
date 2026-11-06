# Immutable plain-object store

This owner stores exact Source bytes and complete Prepared artifacts beneath PlainRootGuard.object_dir using the root owner's same-parent no-clobber publisher, version/kind, exact byte length and maintained BLAKE3.

store_object borrows a slice and read_object verifies the final retained object's length/hash before CONTROL may install a source or preparation head. Old objects remain immutable when a head advances. See [FUNCTIONS.md](FUNCTIONS.md) for explicit Source/Prepared bounds and actual daemon consumers.
