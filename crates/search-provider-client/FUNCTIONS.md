# Public typed client surface

- `NativeClientExpectations::new`: mandatory installation/incarnation/binding,
  pairing reference, exact protocol offer and optional capability comparison.
  Validates protocol shape only; caller owns independent registration provenance.
- `TypedProviderSession::open_native`: consumes one NativeLocalConnection and
  BindingKey, adopts the original Instant, then runs existing pairing/profile.
- `request_standalone_grant`: first post-profile command, exact authenticated
  claims retained/returned by value; missing trust fails before grant I/O.
- `send_request`, `send_cancel`, `receive`, `poll_receive`: one bounded session
  ledger with request/event correlation and absolute per-operation deadlines.
- `close`: drops the sole stream/key/state; idempotent, no remote shutdown or
  claim of rollback.
- `TypedClientError::code/outcome_class`: closed content-free diagnostic and
  conservative evidence for the failed call; unknown never authorizes replay.

Default public transport is concrete. LocalByteStream, SocketIo, SetupBudget,
pairing helpers, State and crypto execution are private. Existing
NativeClientBinding::new, connect_native and from_paired are available only under
nondefault legacy-loopback-compatibility; #332 owns removal. No public token API.
