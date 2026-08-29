# Index family rules

- Qdrant stores rebuildable projections only.
- `search-qdrant-supervisor` owns exact executable identity and local child-process lifecycle.
- `search-qdrant-bridge` owns collection/point/query vendor translation and no process state.
- `search-publication` retires exact point identities and linearizes visibility in the guarded control
  commit; it does not physically reclaim points.
- `search-epoch-pins` owns pins/watermarks; `search-index-reclaimer` performs ordinary exact-ID deletion.
- Security purge is a separate lifecycle path owned by `search-retention`.
- Vendor types never cross public ports and broad correctness-path point updates are forbidden.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put
behavior in the child package that owns the failure state and test seam.
