# Working on subject resolution

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [query cutover](../README.md), package
[README](README.md) and local [source map](FUNCTIONS.md).

The established audit found supplied-candidate resolution functions with package-test callers at the inspected
revision. Treat that as revision-specific evidence. Before deletion, inlining or connection, search current
manifests, Rust/non-Rust callers, constructors, module inclusion, features, targets, tests and workflows. Move
all surviving consumers in the same cut.

Resolve the user's selected entity to actual source locations and expose material ambiguity. Prefer an explicit
source/revision/range, then actual symbol occurrences in the pinned view. Use lexical/name matches only to
nominate alternatives. Preserve repository, document and configuration distinctions; identical names or supplied
equivalence receipts do not establish identity. A truncated candidate set cannot prove uniqueness.

Use the existing manifest and retained-byte lookup. Missing, stale or ambiguous explicit locations remain
visible; do not silently substitute another subject. Apply selected scope and any concrete restricted-session
policy where data is read and returned.

Remove unused resolution receipts, duplicate candidate copies and separate revalidation state with their last
consumers. Reuse ordinary results and the existing window. If only small selection/ambiguity helpers remain,
inline them into query composition and delete the package.

After code, run minimal Rust 1.98 check/Clippy for actual query/preparation/source/protocol consumers. Package
self-tests over supplied candidates are not delivery.
