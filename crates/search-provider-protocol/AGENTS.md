# Working on search-provider-protocol

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [query cutover](../search-query/README.md) and
local [source map](FUNCTIONS.md).

Implement one bounded framed typed exchange used by the actual CLI and daemon:

```text
u32-le length + Serde JSON ClientMessage
→ direct constructed handler
→ optional bounded progress
→ exactly one terminal ServerMessage
```

Keep prefix-before-allocation bounds, incremental reads, request correlation/cancellation, original absolute
deadline and fail-stop behavior after partial output. Encode the complete response frame before the first
write; commit `QueryWindow` insertion/cursor advancement only after complete flush.

Do not preserve or wrap:

- eleven V1 recipes or the parallel V2 endpoint;
- fixed-order manual JSON/base64 grammar;
- pairing challenge/proof, per-frame MAC or shared binding key on child stdio;
- grant, capability descriptor/digest or replay authority;
- binding commit/revoke/drain lifecycle;
- source handle/continuation/security cleanup records;
- legacy tab/hex/TCP/token-file commands.

Trusted child stdio/owner-only local pipes derive connection ownership from actual process/OS handles. A
selected restricted-client adapter authenticates separately and terminates into the same dispatcher; it does
not redefine the baseline wire.

Change current contracts, codec, client, daemon dispatch and CLI together. Do not add a V3 compatibility
family or translate current requests back into old recipes/lines. After code, run minimal Rust 1.98
compiler/Clippy for actual consumers; historical wire/crypto/recipe fixtures are removed with their APIs.
