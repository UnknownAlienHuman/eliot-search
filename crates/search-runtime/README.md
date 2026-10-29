# Runtime family

This directory groups concrete process, OS and storage effects. It is not itself a Cargo package.

| Task | Owner |
|---|---|
| Root classification, initialization, exclusion and finalization | [Root recipe](search-runtime-owner/FUNCTIONS.md) |
| Offline local-object collection, archive and fresh-destination import | [Maintenance recipe](search-retention/FUNCTIONS.md) |
| Online Qdrant generation retirement | [Publication](../search-index-qdrant/search-publication/FUNCTIONS.md) |
| Selected persistent Windows pipe | [Transport](search-provider-transport-windows/FUNCTIONS.md) |
| Accepted encrypted object profile | [Storage profile](../../docs/implementation/revision-encryption.md) |

Plain storage is the baseline. Encryption, credentials and a persistent pipe arrive only with an actual consumer;
none requires a universal key/lease/lifecycle catalogue. Preserve unpublished #598 work.

The [workspace map](../../docs/audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#corrections-to-the-historical-map)
separates main from unmerged package removals. Inspect real callers before removing a boundary. Historical source
sizes and the earlier family inventory remain in the
[prior README](https://github.com/UnknownAlienHuman/eliot-search/blob/623160a77572dac92ca9e19944295a0c9f44119c/crates/search-runtime/README.md),
not as a package-count target or a claim that code is already gone.
