# Function contract

`NativeLocalConnection::open(endpoint, absolute_deadline)` opens exactly the
canonical local Windows client pipe once, with identification SQOS and no wait,
retry or fallback. Reject expired budget and non-byte/server pipe shape.
No public arbitrary handle adoption. Success creates no authority.

Read/Write each perform one synchronous PIPE_NOWAIT operation. Read error 232
becomes WouldBlock, 109/233 becomes EOF. Nonempty zero-progress byte writes
become WouldBlock. Other failures discard raw OS diagnostic text.
Partial I/O consumes only reported bytes; the existing typed engine retains
framing, cancellation, retry quantum and absolute operation deadlines.

Flush is a no-op because this owner has no userspace write buffer. It is never a
peer acknowledgement/durability receipt. Drop closes only this handle and
never rolls back a peer-visible write. Non-Windows opening is explicitly unsupported.
