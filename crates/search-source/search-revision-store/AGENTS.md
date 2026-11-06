# Working on immutable source objects

This owner writes and reads exact plain source bytes beneath PlainRootGuard.object_dir. Use the root owner's existing same-parent no-clobber publication method. The acquired buffer moves directly to this writer; compute maintained BLAKE3 over those exact bytes and verify the final object's actual length and digest by readback before the caller commits current source state.

Do not open the mutable source pathname, start a writer transaction across object I/O, overwrite a conflicting object, create keys/ciphers/receipts, or delete prior immutable references. The connected daemon is the actual caller.
