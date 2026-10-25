# Working on search-handles

Read the [query cutover](../README.md), [query-window map](../search-continuation/FUNCTIONS.md), package
[README](README.md) and local [source map](FUNCTIONS.md).

The established audit found two handle stores, whole-catalogue scans, token records and arbitrary revision-range
expansion at the inspected revision. Treat sizes, topology and caller findings as revision-specific evidence.
Before deletion, search current manifests, Rust/non-Rust callers, constructors, module inclusion, features,
targets, tests and workflows. Move actual protocol/result/daemon callers together.

Normal source-backed cards contain their verified excerpt/location directly. Optional expansion uses
`(window_id, item_index)` inside the same live `QueryWindow` and pinned view.

Move only useful mechanics: exact immutable-range reading and byte/range verification, finite expansion/output
limits, exclusive prepared output and the original absolute expiry/deadline check. Expansion derives a bounded
neighborhood from the exact emitted item's manifest entry; it cannot accept arbitrary absolute revision offsets
or mint another authority record.

Owning-account child stdio needs no cryptographic bearer handle. A selected restricted-client adapter binds and
cancels the window through its concrete session. Remove handle grants, permits, source-catalogue fences,
invalidation generations, durable leases and crypto/provider seams with their last consumers. Delete both old
stores rather than migrating one into the other.

After code, run minimal Rust 1.98 check/Clippy for actual query/window/evidence/protocol/daemon consumers.
Historical handle lifecycle and shape fixtures are removed with their APIs.
