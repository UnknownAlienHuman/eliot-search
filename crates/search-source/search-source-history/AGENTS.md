# Working on source-history removal

Read [README.md](README.md) for the removal boundary and the linked source/object owner recipes.
Do not add history, namespace transitions, registry replay, receipts, compatibility codecs or new callers.

Relocate only values required by actual current consumers, then remove displaced wrappers, exclusive fixtures
and dependencies together. Recheck current manifests, imports, module inclusion, features, targets and non-Rust
callers before deleting a package. Do not retain a facade merely to reexport moved types.

Plain source/head work does not wait for optional #598. Preserve its actual unpublished object/key overlap;
coordinate that overlap without blocking unrelated current-state cutover or creating a second encrypted writer.
Unsupported roots, user originals and sole retained copies remain intact. Source/object/maintenance owners,
not this forwarding package, define current-state guards and retention.

After production changes run minimal locked compiler/Clippy for the actual changed consumers. No broad tests
or compatibility matrix is required to qualify a documentation-only change.
