# Windows provider transport — source map and removal

## Current API

| Source/API | Actual behavior |
|---|---|
| `NativeLocalConnection::open` | Opens one `NativeEndpointNameV1` through `CreateFileW` before/after a caller deadline check. No retry/wait or server ownership. |
| `Read` / `Write` | Delegates to one named-pipe handle in `PIPE_NOWAIT`; maps `ERROR_NO_DATA` to `WouldBlock` and broken-pipe errors to EOF/failure. |
| `flush` | Returns success without calling `FlushFileBuffers`; it is not a peer acknowledgement. |
| platform support | Windows client only; non-Windows always returns unsupported. |
| production callers | `search-provider-client` typed I/O/native session, its example and CLI `provider_client/local.rs`. These belong to the old pairing/grant client stack. |
| missing half | No named-pipe server/listener or actual current daemon dispatcher is implemented in this package. |

The package uses the old protocol's endpoint type and has no independent product responsibility once that
client stack is removed.

## Current disposition

Baseline local Search uses the child process and inherited stdio:

```text
CLI owns child/stdin/stdout
→ one current framed client
→ one current daemon dispatcher
```

Delete:

```text
crates/search-runtime/search-provider-transport-windows/
workspace member / target dependency edge
old typed-client LocalByteStream implementation
CLI native-local constructor which consumes it
transport examples/fixtures tied to pairing/grant expectations
```

Do not move this API into another forwarding package.

## Optional future pipe

Only a real persistent-daemon requirement may add a Windows named pipe. Implement server and client together
against the current framed protocol. The platform owner must:

- create a private endpoint under the current installation/root identity;
- apply and verify the selected owner-only ACL/peer identity;
- retain exact listener/connection handles;
- bound accept/read/write/backpressure/cancellation under one absolute context;
- close and release on disconnect/shutdown;
- expose only an owned byte stream to the current dispatcher/client.

Use ordinary blocking/overlapped I/O according to the measured serving model. Do not inherit `PIPE_NOWAIT`
polling merely for compatibility. No provider pairing, application MAC, grant or capability digest is needed
for the same-owner profile; a genuinely restricted external profile adds its authentication separately.

## Verification if reintroduced

Exercise wrong-user/foreign server rejection, pipe type/mode, partial prefix/body I/O, backpressure,
cancellation, EOF, partial output, listener shutdown and actual CLI/daemon round trip. A client-only open test
or existing source compilation does not establish a supported transport.
