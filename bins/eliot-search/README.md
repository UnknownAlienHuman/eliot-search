# eliot-search

**Standalone CLI and local client.**

**Status:** substantial command, client, bootstrap, credential, paging and provider-session source exists.
Current commands include development/DIRECT health, one-shot scan, file/directory ingestion, directory
sync, retained-root search, source listing, revision readback and maintenance operations. The final
canonical named-pipe/provider path, real indexed recipes and installed-product qualification remain
incomplete.

## Owns

- argument/help/version handling;
- local daemon bootstrap and connection UX;
- bounded request construction;
- newline-delimited JSON/result rendering;
- pagination, cancellation and handle/continuation client behavior;
- health, doctor and maintenance command surfaces;
- standalone corpus/portfolio management UX when #218 lands;
- optional generic agent-adapter invocation/discovery when #219 lands.

## Must not own

- opening redb, CAS or Qdrant directly;
- reimplementing query planning, access, ranking or candidate validation;
- minting unbounded grants or inferring authority from local-user/loopback state;
- exposing raw Qdrant collections, filters, cursors or point IDs;
- rendering hidden source/vendor payloads;
- silently treating legacy TCP/token-file development paths as the final transport.

## Current boundary

The CLI is not `unimplemented`, but it is not a qualified release client. Existing DIRECT commands are
development evidence and must not be described as complete Qdrant-backed agent analysis. The final product
must use the canonical daemon/provider protocol, support real indexed recipes, portfolio selection,
free-text/orientation and exact source-backed expansion without a second implementation path.

- **Product area:** Architecture S32/S33
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Current status matrix:** [../../docs/product/IMPLEMENTATION_STATUS.md](../../docs/product/IMPLEMENTATION_STATUS.md)
