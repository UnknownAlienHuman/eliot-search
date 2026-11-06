# Qdrant bridge

The concrete Backend privately owns one reusable tonic channel and one Tokio runtime over the exact
Qdrant 1.20.0 SDK's public generated clients. SDK/transport types and actual credentials remain inside
this crate. [FUNCTIONS.md](FUNCTIONS.md) owns the selected physical/query profile and acceptance rules.

Publication supplies fixed complete PointInput pages. Pure batch_request_bytes measures their actual
SDK protobuf size before persistence, and insert_only repeats the same checked encoding with InsertOnly
and wait=true. Exact schema/point/count readback and a real TextQuery canary precede publication's
CONTROL commit. Query returns only point/source routing and finite server score; retained objects supply text.
The gRPC point uses the SDK's required present named-vector map with zero entries, not an omitted container
or any dense/sparse vector values; actual size measurement includes its framing.

The operator owns Qdrant startup/storage/shutdown. This crate has no supervisor, capability/qualification
registry, auth lease, sparse/local BM25 encoder, mutation ledger or model fallback. UnknownDispatch leaves
publication's target unserved until its real uncertain-effect rules permit progress.

[Lexical policy](../../search-lexical/FUNCTIONS.md) defines the fixed Code/BM25 semantics;
[publication](../search-publication/FUNCTIONS.md) owns durable pages, current/evidence and view installation.
Source check, dependency resolution and native acceptance are separate evidence recorded by the designated
verifier; this README alone does not claim that SDK/server operations have executed successfully.
