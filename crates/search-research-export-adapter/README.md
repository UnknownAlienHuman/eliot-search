# search-research-export-adapter

**Status:** optional design only. No Cargo package, runtime handler or capability exists.

Implement this adapter only when one concrete Research consumer requires the `eliotr.normalized.v1` bundle. It receives already selected, source-backed materialization data from the daemon and returns one bounded wire object. The adapter does not create another storage, authority or qualification layer.

## Owns when implemented

- mapping the selected materialization into the exact wire schema;
- deterministic manifest assembly and explicit unknown-field rejection;
- wire-level digest calculation and readback;
- reporting unsupported or lossy fields without fabricating completeness.

## Does not own

- source admission, materialization or publication;
- ownership transfer, cutover receipts or canonical Research writes;
- Qdrant, redb, object-store or key access;
- unsaved overlays unless a concrete consumer and provenance contract are selected;
- provider registries, activation gates or release authority.

The concrete operation belongs in [FUNCTIONS.md](FUNCTIONS.md) after a real caller is selected. Until then this directory is documentation, not delivered functionality.
