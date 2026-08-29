# Family rules

- Qdrant stores rebuildable projections only.
- Vendor transport is isolated in search-qdrant-bridge.
- VisibleEpoch changes only in the guarded control-journal commit.
- Exact point manifests replace broad correctness-path updates.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put behavior in the child package that owns the failure state and test seam.
