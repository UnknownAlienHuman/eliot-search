# Working on search-continuation

Read the [query cutover](../README.md), [daemon map](../../../bins/eliot-searchd/FUNCTIONS.md) and local
[source map](FUNCTIONS.md).

Do not repair or integrate the shared continuation lifecycle. The product already has a second daemon
catalogue, and neither model owns the pinned `PublishedView`. Replace both with one process-local
`QueryWindow` in the real query owner.

Keep only:

- final ordered `EvidenceRef[]`;
- direct cursor slicing;
- one `PublishedViewGuard`;
- finite TTL/item/metadata/read/output limits;
- optional bounded cached excerpts/item locators;
- exclusive prepared-page delivery which commits insertion/advance only after complete output.

Do not keep durable replan descriptors, issued fingerprints, token digests, grant/security generations,
cleanup effects, source-catalog fences or terminal histories. Restart may expire every ordinary interactive
window. A real restricted-client adapter binds/cancels its own windows directly.

Expansion uses `(window_id, item_index)` and a bounded neighborhood around that exact manifest range. It may
not request arbitrary bytes from the revision or mint another handle.

Change protocol/results/daemon callers together, move the useful prepared-page code and delete this crate
and the legacy daemon catalogue. After code, run minimal Rust 1.98 compiler/Clippy for actual query/window/
protocol consumers. Broad lifecycle fixtures for deleted APIs are not implementation blockers.
