# Handle removal and emitted-item expansion

The [prior source audit](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-query/search-handles/FUNCTIONS.md)
found two stores, copied source/revision records, random tokens, whole-catalogue scans and unconstrained revision
ranges. That is revision-specific evidence. Recheck current Cargo/Rust/non-Rust callers before deletion.

## Replacement boundary

There is no public handle mint. A card may carry WindowItemRef `{ window_id, item_index }` only for the same live
session. This is a locator; the window's ordered EvidenceRef and pinned view provide its evidence binding.
One-shot output provides no reusable locator.

The [window recipe](../search-continuation/FUNCTIONS.md#operation-ownership) owns serialization and pending output.
`prepare_expand` validates a successfully emitted item, fixed expiry and the real session binding before retained
I/O. It moves/owns the needed operation state without retaining a shared mutable window borrow or mutex.

Derive a finite relative neighborhood around that item's representation/range, reserve the original operation's
read/output budget, and use [evidence verification](../search-candidate-validator/FUNCTIONS.md). No arbitrary
absolute revision offsets, source-catalogue revalidation, latest-source substitution or independent handle TTL.
Only complete output commits new progress; damaged/partial output closes the session. Ordinary healthy request
failure follows the window owner's restoration rule, not a second handle lifecycle.

## Delete with consumers

Remove both handle stores, entropy/token hashing, copied authority records, catalogue scans/fences, permits,
invalidation generations, durable leases and arbitrary-range expansion. Move only useful range verification and
prepared-output commit into the window/evidence owners. Serialization does not mean holding a mutable borrow
through I/O. Delete exclusive fixtures and the crate once its callers move.

After code run minimal pinned check/Clippy for real query/window/protocol/daemon consumers. Later cases exercise
only emitted items, bounded neighborhoods, wrong/missing bytes and expiry/disconnect/restart behavior.
