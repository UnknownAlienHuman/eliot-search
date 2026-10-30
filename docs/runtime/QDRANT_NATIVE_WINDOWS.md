<a id="qdrant-on-windows"></a>
# Local Qdrant on Windows — status and update route

## Selected implementation baseline

Main still pins client 1.19.0. The selected scoring-text path targets one exact server/client 1.20.x patch, adopted
with the real consumer. This page provides no verified ELIOT installation/config command or Windows support claim.
[Bridge dependency cut](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#dependency-cut) owns the
SDK/server choice and gRPC endpoint; [root Quickstart](../../QUICKSTART.md) owns executed product commands.

## Baseline product mode

The operator starts Qdrant on the same machine and controls its private data directory and listeners. ELIOT
connects through the [local bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#local-connection).
Do not add a mode selector, launcher, server download, generated credential or configuration writer to the app.
A pre-existing key is passed privately only when required. Client loopback selection does not configure server
listeners; deployment must keep them local. No remote/anonymous fallback on connection failure.

The previous illustrative ELIOT TOML was not an implemented schema. Do not copy it into production settings.
Add typed endpoint input only with its actual bridge consumer; config fields without consumers are rejected.

## Windows evidence required for Qdrant 1.20

Once the connected product exists, record exact server origin/version/bytes, Rust client version, platform and
commands. Verify the stage's actual collection/schema, first build/query, retained excerpt and honest failures.
Restart/old-view/cleanup/resource cases join their delivery stages; do not run a broad qualification platform as
a prerequisite for writing the first path. Linux API/source inspection is not Windows execution.

<a id="optional-managed-child-mode"></a>
## Deferred managed-server convenience

The [supervisor removal map](../../crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md) owns the old
source boundary. A later explicitly selected launcher must not revive its leases, receipts or state machines.
No managed server is needed to establish a working connection or publish a generation.

## Qdrant Edge is separate

Any future embedded evaluation is a replacement experiment, not a second backend or fallback. Recheck current
upstream evidence only for a real adoption task; historical release observations do not select an implementation.

## Updating Qdrant

| Change | Update and check together |
|---|---|
| Client/server patch | Consumer Cargo entry, lock, exact server artifact/version and donor evidence; compile actual bridge and consumers. |
| Schema or retrieval meaning | Physical profile and fresh-generation publication; never relabel old data. |
| Request-only scoring | Query-profile consumer; do not rebuild for k/b alone. |
| Working Windows command | Root Quickstart plus registry revision/platform/limits, after executing it. |

Do not auto-upgrade or reuse an incompatible data directory. Preserve old objects and installed data during
side-by-side evaluation; only the publisher may retire ELIOT's eligible collections.

## Historical 1.19 development artifact

[The recorded September smoke and exact artifact identifiers](https://github.com/UnknownAlienHuman/eliot-search/blob/7ed46f79f07a23f01f75f5e85a96248dc42e280b/docs/runtime/QDRANT_NATIVE_WINDOWS.md#historical-119-development-artifact)
remain historical evidence. They do not establish present 1.20 availability, current-main integration or support.
