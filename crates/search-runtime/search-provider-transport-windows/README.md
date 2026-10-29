# Windows provider transport — remove the old client path unless a real consumer remains

This package is a small synchronous client-side named-pipe adapter for the old provider protocol. It is not a
server/listener and is not the selected baseline transport; the current local product starts with owned child
stdin/stdout and one framed protocol.

## Revision-specific evidence

At the inspected public-main baseline, production source was about 8.4 KiB before tests and callers were the old
typed provider client, an example and an unconnected CLI pairing path. No caller was found on the selected
child-stdio path. These size and caller findings are historical evidence, not permanent current-branch truth.

Before deletion, repeat an exact current search across manifests, Rust imports/constructors, non-Rust tooling,
feature/target wiring, tests and workflows. If no real persistent-daemon consumer exists, remove this package,
its workspace/dependency/feature entries and old client callers in the same cut. Do not retain it merely for a
hypothetical future pipe.

If a current consumer exists, keep only the smallest truthful unavailable boundary while it moves; do not
harden legacy pairing/grant/session semantics.

## Optional future persistent transport

Only after a measured product need, implement one actual owner-only Windows named-pipe server and client against
the current framed protocol:

```text
real server/listener with selected ACL and peer-owner checks
+ one owned bounded client stream
→ current u32-length + Serde messages
```

Transport reachability does not grant source scope. The daemon resolves every request against its pinned view
and any selected restricted-session policy. Do not restore `NativeEndpointNameV1`, endpoint files, pairing keys,
grants, capability digests, TCP/token-file fallback, discovery framework, reconnect pool or generic transport
plugins.

After deletion or replacement, run minimal Rust 1.98 check/Clippy for actual protocol/client/CLI consumers.
Native pipe runtime cases return only with a selected persistent-pipe implementation.
