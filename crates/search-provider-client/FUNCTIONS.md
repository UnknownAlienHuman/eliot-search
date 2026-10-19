# Provider client — source audit and replacement map

## Current package

Known production source:

```text
lib.rs                  786 B
typed.rs             14,513 B
typed/io.rs          11,724 B
typed/native.rs      19,575 B
typed/state.rs       13,733 B
typed/state/grant.rs 10,511 B
```

This is about 70.8 KiB before fixtures/tests. `typed/state/sequence_fixtures.rs` alone adds another 42,994
bytes of fixture source.

Current public API:

| API | Current behavior |
|---|---|
| `NativeClientExpectations::new` | Requires installation/incarnation/binding/pairing/protocol and optional capability coordinates from another trusted producer which is not connected for baseline local use. |
| `TypedProviderSession::open_native` | Consumes a native stream plus non-clone binding key, then runs pairing/profile. |
| `request_standalone_grant` | Mandatory first product command before ordinary requests. |
| `send_request` / `receive` / `send_cancel` | Useful correlation/cancel/event ordering wrapped in sequence/MAC/grant state. |
| legacy-loopback feature | Retains old TCP signatures for a harness; not the supported product route. |
| active CLI | Reexports this client but its `ProviderSession` still wraps the separate legacy core and maps indexed query back to a marked tab/hex command. |

The target is not another extraction or adapter between these clients. Replace the active CLI and typed
client with one current session, then delete the unused constructors and legacy core.

## Target internal state

```text
LocalClient {
  owned connection
  limits
  next_request_id
  live requests: finite map only when multiplexing is enabled
  closed / failed state
}
```

Sequential baseline needs only one live request. It does not retain protocol history after terminal/close.

Useful methods:

```text
invoke(operation, timeout) -> Result<OperationResponse, ClientError>
start(operation, timeout) -> RequestId        // optional
cancel(request_id)                            // optional
receive_next() -> Progress | Terminal         // optional
close()
```

### Write

- construct current `ClientMessage` directly;
- validate current typed field/collection limits;
- Serde-encode under maximum body bytes;
- write prefix/body/flush under the original absolute deadline;
- any partial write makes the session unusable.

### Read

- incrementally read and validate the four-byte prefix before allocation;
- read exactly the bounded body under the same deadline;
- Serde-decode current `ServerMessage`;
- reject wrong request ID, duplicate terminal, progress after terminal, unknown version/event and oversize
  output;
- EOF before a complete terminal is a failed request, not success/rollback evidence.

### Cancel

Cancellation targets one live ID on the same connection and shares the original/explicit finite deadline.
Disconnect closes all live requests. No cancellation receipt or terminal history persists.

## Error surface

Expose stable local classes needed by the CLI:

```text
Usage / Encode / Decode / Version
ConnectionUnavailable / Timeout / Cancelled
ProtocolViolation / OutputIncomplete
ServerFailure { stable code, partial/retry metadata }
```

Keep query text, source content, credentials, raw OS/vendor errors and frame bytes out of default Debug/Display.
Do not flatten a server partial/gap response into transport failure.

## Connection constructors

- child stdio: consumes owned child/stdin/stdout handles and records child ownership for bounded cleanup;
- owner-only pipe: consumes a stream already admitted by the platform/OS owner;
- restricted external client: selected adapter authenticates first, then hands the common framed stream and
  session metadata to the same engine.

No endpoint-file/token-file/TCP fallback. No possession of a pipe name or shared key manufactures restricted
scope.

## Delete after caller cutover

- all pairing/profile/grant state and fixtures;
- `NativeClientExpectations`, `NativeClientBinding`, `BindingKey` requirement for trusted local use;
- exact old profile/sequence/MAC byte compatibility;
- legacy-loopback constructors/features;
- duplicate CLI `provider_client/core.rs` session/codec/endpoint implementation;
- unused trusted native registration/credential constructors unless a real restricted consumer remains;
- examples/fixtures whose only purpose is exercising removed grants/wire records.

Keep/rewrite only finite framed I/O, correlation, cancellation, one-terminal validation, absolute deadlines
and owned connection cleanup.

## Verification

- child stdio and selected owner-only pipe constructor ownership;
- prefix split across reads and body split across reads;
- oversize prefix before allocation;
- partial write/read/EOF fail-stop;
- one terminal, bounded progress and cancellation order;
- no deadline renewal;
- owned child cleanup on success/error/timeout;
- current typed Search/Continue/Expand result decoding and useful CLI exit mapping.
