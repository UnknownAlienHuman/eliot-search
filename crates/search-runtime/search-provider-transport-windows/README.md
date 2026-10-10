# Windows provider byte transport

Concrete OS effect owner used by the CLI's existing private typed stream seam.
No session or protocol logic is duplicated here. #380 implements the source
foundation; #373/#235 still own the reusable API and engine extraction, and #332
owns native server, command cutover and compatibility removal.

Synchronous PIPE_NOWAIT polling is deliberate; it is not overlapped or async I/O.
If an empty/closed pipe reports ERROR_NO_DATA, the engine retains the original
deadline and conservative unknown outcome. Successful open/flush is not evidence
of pairing, grant acceptance, peer receipt or rollback.

Spec references (verified 2026-10-10):

- [Microsoft pipe modes](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes)
- [GetNamedPipeInfo](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-getnamedpipeinfo)
- [SetNamedPipeHandleState](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-setnamedpipehandlestate)
- [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- [Rust 1.98 Windows File implementation](https://github.com/rust-lang/rust/blob/1.98.0/library/std/src/sys/fs/windows.rs)

Focused pure/deadline fixtures are AUTHORED_NOT_RUN. Native wrong-user, foreign
server, pipe mode, backpressure, cancellation, EOF and installed end-to-end
qualification remains unperformed Q/#332/#215.
