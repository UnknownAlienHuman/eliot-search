# Immutable plain-object store

This owner stores the exact acquired source buffer beneath the existing PlainRootGuard object directory. It uses the root owner's same-parent no-clobber publisher, a format-versioned source ObjectRef, exact byte length, and BLAKE3 content address.

store_source_bytes publishes the original allocation, then read_source_object reads the final object back into a bounded buffer and independently verifies its length and BLAKE3 digest before control may install a source head. Old objects are immutable and remain available when a current head advances.
