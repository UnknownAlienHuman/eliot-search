# Working on search-continuation

Read the [query cutover](../README.md), [daemon map](../../../bins/eliot-searchd/FUNCTIONS.md), package
[README](README.md) and local [source map](FUNCTIONS.md).

The established audit found a shared continuation lifecycle and a second daemon catalogue, neither owning the
pinned `PublishedView`, at the inspected revision. Treat topology and caller findings as revision-specific
evidence. Before deletion, search current manifests, Rust/non-Rust callers, constructors, module inclusion,
features, targets, tests and workflows. Move actual protocol/result/daemon callers together.

Replace both models with one process-local `QueryWindow` in the real query/session owner. Keep only final ordered
evidence references, direct cursor slicing, one `PublishedViewGuard`, finite TTL/item/metadata/read/output limits,
optional bounded cached locators and prepared-page delivery that advances only after complete output.

Do not keep durable replan descriptors, issued fingerprints, token digests, grants/security generations, cleanup
effects, source-catalogue fences or terminal histories. Restart may expire ordinary interactive windows. A real
restricted-client adapter binds/cancels its own windows directly.

Expansion uses `(window_id, item_index)` and a bounded neighborhood around the exact pinned manifest range. It
cannot request arbitrary revision bytes or mint another handle.

Move useful slicing/output mechanics and delete the shared crate plus legacy daemon catalogue after caller
cutover. After code, run minimal Rust 1.98 check/Clippy for actual query/window/protocol/daemon consumers;
historical lifecycle fixtures disappear with their APIs.
