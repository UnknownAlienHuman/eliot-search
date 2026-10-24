# Runtime family rules

Read the root [architecture](../../docs/architecture/README.md),
[implementation](../../docs/implementation/README.md), family [README](README.md) and
[storage-profile note](../../docs/implementation/revision-encryption.md).

A runtime package exists only for a real process, OS or storage effect with a distinct owner/failure boundary.
Use concrete owner APIs; do not create a generic secret, lease, retention, migration or effect platform.

## Current ownership

- `search-runtime-owner` retains the real data-root/process guard while current stores are open. Remove
  owner-epoch/lease/receipt ceremony that no direct consumer needs.
- `search-retention` is already removed as a Cargo package. Its remaining documentation owns **stopped-root
  local-object collection and archive/import behavior only**.
- Qdrant point/generation retirement is online publication cleanup; it does not belong to local-object
  maintenance or a restored generic retention package.
- Current/pending/published/cleanup references determine what offline maintenance must mark, but they do not
  make hot-path local object sweeping safe.
- `search-provider-transport-windows` survives only if a selected persistent owner-only pipe remains after
  child-stdio integration; otherwise remove it with callers.
- Secret/key code is profile-specific. Qdrant, restricted clients and encrypted storage do not share a
  universal catalogue/lifecycle.
- Plain local storage is a valid current-format profile. An encrypted profile keeps one concrete OS-protected
  key owner and authenticated object codec inside the immutable-object boundary.

Historical no-caller/package-size findings are revision-specific. Before deleting `search-revision-crypto`,
transport or secret packages, recheck exact current manifests, Rust/non-Rust callers, features, tests and
workflows, then coordinate actual #598 overlap. Do not inspect or compete with its private object/key writer.

Remove unconnected pairing composition, secret leases, operation receipts, migration codecs and compatibility
consumers with their last real callers. Do not rebuild current/candidate/retiring key roles or cross-root
re-encryption without a concrete selected operation.

After code, run minimal Rust 1.98 compiler/Clippy for changed direct owners/consumers. Old lifecycle, pairing,
rotation, receipt and compatibility matrices leave with the APIs they protected.
