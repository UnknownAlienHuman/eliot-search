# search-provider-protocol — current API and cutover

## Useful existing mechanics

| Source/API | Keep |
|---|---|
| `frame.rs::FrameCodec` | Length-prefixed finite frame and malformed/size checks. |
| `frame/transport.rs::TypedRecordBuffer` | Bounded partial-read assembly; remove its unused proof digest. |
| request/in-flight lifecycle | Request correlation, one deadline/cancellation and single terminal cleanup. |
| typed client/server event shape | A basis for the one replacement format. |

## Parallel machinery to remove from the local path

- V1 codecs for eleven frozen `RecipeIdV1` bodies/results.
- Separate V2 evidence/orientation codec family.
- manual fixed-order JSON grammars and canonical re-encode proofs.
- pairing/grant/body-MAC/replay ceremonies in trusted-local constructors.
- capability descriptor projection treated as availability or permission.
- CLI tab/hex framing and substring event parsing.

## Replacement exchange

Read a `u32` length, reject above the configured ceiling before allocating the body, read exactly that
many bytes, then decode one typed request with `serde_json::from_slice`. Encode one bounded response/event.
`from_reader` on a persistent connection is inappropriate because it waits for EOF.

Current request kinds are `Search`, `Exact`, `Continue`, `Expand` and optional `Orient`/`Compare` when
handlers exist. The handler table belongs to daemon composition. The local client requires no advertised
registry; a restricted adapter may derive an informational operation/version/limit list from that table.

Absence of a handler is a typed unsupported result. It is not repaired by a descriptor, qualification
receipt, default capability boolean or fallback to a semantically different legacy command.

## Behavior to check

Through real callers: partial/oversized frames, malformed typed bodies, request IDs, cancellation,
deadline propagation, finite queued/output bytes, disconnect cleanup, exactly one terminal response and
truthful unsupported operations. Run credential/replay/scope tests only for selected restricted clients.
