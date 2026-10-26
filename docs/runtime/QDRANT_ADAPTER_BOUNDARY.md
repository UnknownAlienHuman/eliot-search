# Qdrant adapter boundary

Qdrant Server/Rust client **1.20.x** is the selected indexed backend for the first connected product. The repository
lock still contains client 1.19.0; upgrade client, lock, schema builders and actual publication/query consumers in
one code cut. `search-qdrant-bridge` is the only package allowed to depend directly on `qdrant-client`; vendor types
remain private.

## Selected runtime boundary

Daemon composition supplies one configured endpoint, actual optional API key, bounded transport settings and one
absolute deadline/cancellation. One daemon-owned client/runtime serves publication and query. Startup observes live
server capabilities and verifies the exact selected physical collection/schema; build hashes and probe reports are
development/release evidence, not constructor credentials.

The baseline Qdrant 1.20 retrieval profile is:

```text
one retrieval-text payload field
+ scoring full-text payload index
+ explicit code tokenizer/text-processing settings
+ TextQuery with query-time k/b
+ payload scope filter and independent idf.corpus base population
```

Do not generate baseline sparse BM25 vectors, query vectors or local fixed `avg_len`. Exact punctuated identifier
search remains independent of tokenized BM25.

## Physical view identity

`PublishedView` names the exact physical collection and matching redb snapshot. Query addresses that collection
directly. Qdrant aliases are not serving, recovery or cleanup authority because alias mutation cannot atomically
install ELIOT evidence or preserve old pinned views.

Stage 2/3A visible collections are immutable. Any selected-state difference builds another never-reused physical
collection. Point-level mutation and stable occurrence identity exist only in Stage 3B when Stage 3A fails declared
release budgets.

## Scope and sharding

BM25/full-text statistics are physical-shard-local. `idf.corpus` changes the statistics population on each shard;
it cannot reach data in another shard. Do not split a general portfolio by root/source custom shard key or merge
independently scored shard lists locally. Custom sharding is considered only for a scope always queried as one
partition and only after measurement.

## Current code debt

The former `QualifiedGate` constructor input was removed at `7d92fb8b`; do not recreate it. Current
`RealDataPlane::connect` still requires supervisor binding and an API-key lease, checks an exact server build and
uses fixed nested timeouts. Other qualification/auth-binding wrappers, operation receipts and duplicated
point/payload models remain. Normal daemon publication/query is not connected.

The #263 cutover preserves real auth, transport, schema, bounded response and exact readback while deleting
qualification/binding/receipt authority and unused SDK wrappers after current callers move.

## Optional process and Edge boundaries

External endpoint mode is baseline. A later managed-child profile may own one actual process handle, private
resources, real platform child-tree guard and bounded startup/shutdown. It uses this same bridge for readiness.

Qdrant Edge is a separate embedded-backend replacement candidate, not a managed child or fallback. Evaluate a
newer released Edge line only after the server path works; it must replace this bridge cleanly after persistence,
Windows, BM25/filter, restart and size/latency evidence.

## Verification

Run 39872058276 passed Rust 1.98 locked check/strict Clippy for `search-qdrant-bridge --lib` at `7d92fb8b` only.
It covers neither current consumers nor Qdrant 1.20.

For the real cut run bridge + publication/query/daemon check/Clippy, then live 1.20 scoring-full-text schema,
code-tokenizer, TextQuery, scope/IDF, insert-only readback, physical-generation restart, cancellation and
collection-delete/absence cases. Record exact server/client/profile and raw results in the registry/PR.
