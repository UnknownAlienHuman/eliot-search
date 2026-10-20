# Runtime family rules

Read the root [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md), family [README](README.md) and
[storage-profile note](../../docs/implementation/revision-encryption.md).

- A runtime package exists only for a real OS/process/storage dependency or distinct failure state.
- Use concrete owner APIs. Do not create a generic secret, lease, retention, migration or effect platform.
- `search-runtime-owner` retains actual root/process custody; remove owner-epoch receipts which no direct
  consumer needs.
- `search-retention` is deleted after current/pending/pinned roots own GC.
- `search-provider-transport-windows` remains only for a selected persistent owner-only pipe; child stdio is
  the first local protocol path.
- Secret/key code is profile-specific. Qdrant, restricted clients and encrypted storage do not share a
  universal catalog/lifecycle.
- Plain local storage is a valid fresh-root profile. Encryption is selected, not assumed for every object.
- An encrypted profile keeps one concrete OS-protected key owner and authenticated object codec inside the
  immutable object boundary. No generic current/candidate/retiring key roles or cross-root re-encryption.
- Delete `search-revision-crypto` if the accepted #598 handoff does not directly use it.
- Remove unconnected pairing secret composition, secret leases, operation receipts, migration codecs and
  historical compatibility consumers with their last callers.

Preserve #598's active private work. Do not inspect, reset, rebase, merge or implement a competing key/object
writer. Update public callers only after its accepted handoff identifies the useful current-format seam.

After code, run minimal Rust 1.98 compiler/Clippy for changed direct owners/consumers. Old lifecycle, pairing,
rotation, receipt and compatibility matrices are deleted with their APIs rather than maintained as gates.
