# Runtime family

Keep only concrete process/file effects required by the [local product](../../docs/architecture/LOCAL_SCOPE.md).
This directory is organizational; it does not require a runtime framework or one crate per effect.

| Work | Owner |
|---|---|
| Root, process lock, small no-clobber helper | [Root](search-runtime-owner/FUNCTIONS.md) |
| Owned child and I/O lifetime | [Client](../search-provider-client/FUNCTIONS.md) |
| Stopped-root local collection/archive/import | [Maintenance](search-retention/FUNCTIONS.md) |
| Online Qdrant retirement | [Publication](../search-index-qdrant/search-publication/FUNCTIONS.md) |

Secret/crypto packages, pairing and optional persistent-pipe infrastructure are legacy removal targets, not
required services. Current Cargo/source still contains old implementations; documentation does not remove them.
Follow the [removal map](../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring) with real callers.
No encryption handoff blocks plain storage. Preserve user data and unpublished work.
