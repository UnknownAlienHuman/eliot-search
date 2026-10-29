# search-point-identity

Derive a deterministic point address and retain enough full identity to reject collisions. The address is a
locator, not complete evidence. [FUNCTIONS.md](FUNCTIONS.md) owns the key and stage boundaries.

Stage 2/3A uses generation-bound IDs and may remint every point in the next complete generation. Occurrence
matching/incarnations and stable reuse are conditional Stage 3B work, not prerequisites for the first result.
Exact revision/range comes from pinned evidence.

Move identity, projection, readback and evidence consumers together. Keep no registry, compatibility issuer or
forwarding crate; inline a trivial remaining helper into publication/preparation.
