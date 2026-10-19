# Working on search-source-provenance

This package is a legacy original-input/archive compatibility owner scheduled for deletion. Read
[README.md](README.md) and the current [admission map](../search-source-admission/FUNCTIONS.md).

Do not add fragments, archive profiles, receipt reconstruction, storage proofs, current ingestion callers,
forwarding APIs or compatibility promises. Do not optimize the v4 restore path.

Migrate each caller to current typed state:

- native acquisition keeps the exact body and small final admission outcome;
- source registry stores only current root/source/head references;
- restart point-reads current redb state;
- unsupported old roots remain intact and unavailable.

Delete this package, source-record/registry original-input codecs and their fixtures after the final caller
moves. Preserve #598's active private work and use only its accepted handoff.

After production changes, run minimal Rust 1.98 compiler/Clippy for the actual source/admission/registry/
daemon consumers. Archive byte parity and provenance fixtures are removed with their APIs.
