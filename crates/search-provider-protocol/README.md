# search-provider-protocol

Bounded typed request/result framing for the CLI and daemon. This package performs no source, database,
Qdrant or secret-store I/O.

## Target local exchange

Use one current request/result family from `search-contracts` over owned child stdio or an owner-only
local pipe:

```text
length-prefixed bounded frame
→ typed request ID + Search | Exact | Continue | Expand | optional Orient/Compare
→ progress only when useful
→ one terminal SearchResponse or typed error
```

Reuse frame-length rejection, correlation, cancellation, finite in-flight work and terminal cleanup.
Replace the CLI's tab/hex protocol, substring event parsing, manual JSON grammar and parallel V1/V2 codecs
with Serde `from_slice` / bounded serialization in the existing owners.

## Handler availability

The local CLI calls constructed handlers directly and needs no capability handshake. An unsupported
operation returns a typed error. For a selected restricted-client profile, advertisement is derived from
the actual handler table and may include request versions and limits. It is informational; real session
credentials and delegated scope are checked independently on each request.

Do not preserve `SearchProviderCapabilityDescriptor` as a permission credential, `CapabilityRegistryRevision`,
eleven-recipe parity tables, qualification receipts or canonical descriptor digests. No handler exists
because a descriptor says it does.

## Current source to simplify

- `FrameCodec` and `TypedRecordBuffer` provide useful bounded framing.
- request guards/in-flight state provide useful correlation/cancellation.
- V1 recipe codecs freeze eleven request/result families.
- V2 evidence codecs form a parallel endpoint.
- pairing, grants, body MACs and replay transcripts belong only to selected restricted clients.
- daemon `CanonicalRecipeRegistry` / `CanonicalQueryHost` / registered-service composition have no real
  registered recipe-handler implementations in public main.

Change protocol, dispatch, handler and client together. Delete superseded recipe/capability/session layers
from the trusted-local path rather than fabricating credentials for them.

## Verification

Check prefix/size rejection before allocation, partial reads/EOF, typed decode, request correlation,
cancellation, output bounds, one terminal result and unsupported-handler errors through the actual
client/daemon connection. Restricted-client credential/replay tests run only for that profile.
