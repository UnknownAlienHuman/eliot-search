# Rust syntax enrichment qualification

This directory defines the P10 evidence contract for one exact Rust syntax parser/grammar/dependency
profile used by `search-code-enricher`.

- [`artifact.toml`](artifact.toml) — exact dependency/profile identity; initially unqualified.
- [`probes.toml`](probes.toml) — mandatory behavior, safety, span and assurance probes.

No parser or grammar is selected by these files. A dependency becomes eligible only after exact source,
version/checksum/license and every mandatory fixture receive immutable raw evidence and independent
review.

Compilation, upstream popularity or parsing one valid file is insufficient. Failure or `UNAVAILABLE`
keeps Rust structure enrichment disabled while ordinary text/code retrieval may remain available.
