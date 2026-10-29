# Working on search-result-projector

Read the [query cutover](../README.md) and local [source map](FUNCTIONS.md).

Do not connect the current raw-score/handle projector. Retrieval establishes final order; evidence resolution
establishes actual source bytes; `QueryWindow` owns paging and optional item locators. Projection only formats
already ordered `VerifiedEvidenceItem` values under remaining output limits.

Preserve input order. Charge limits before copying/encoding excerpts. Include actual verified source,
revision, representation, location and excerpt. Keep gap details bounded and report aggregate omissions.
Never mint handles, compare raw backend scores, accept caller proof Booleans or infer complete-negative
coverage from top-k.

Change current protocol/result consumers directly and delete old projector types/fixtures. If formatting is
small, inline it into query/protocol composition and remove this crate rather than maintaining another
boundary.

After code, run minimal Rust 1.98 compiler/Clippy for actual query/daemon/protocol consumers. Broad result
fixtures follow the connected retained-evidence path.
