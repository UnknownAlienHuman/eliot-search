# Runtime family rules

Read the affected owner's recipe, not every runtime package. Keep a package only for a real process, OS or
storage effect; do not create a general lease, secret, retention, migration or effect platform.

[Root](search-runtime-owner/FUNCTIONS.md) owns process-lifetime exclusion and root classification.
[Maintenance](search-retention/FUNCTIONS.md) owns stopped-root local collection/archive/import;
[publication](../search-index-qdrant/search-publication/FUNCTIONS.md) owns online whole-generation Qdrant cleanup.
Plain storage needs no secret subsystem. Read the [encrypted-profile note](../../docs/implementation/revision-encryption.md)
only for an actual accepted encrypted-storage consumer; preserve unpublished #598 work.

Package removal status is branch-specific; the [workspace map](../../docs/audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md#corrections-to-the-historical-map)
distinguishes the unmerged refactor from main. Before deleting code, inspect current manifests, Rust/non-Rust
callers, features, tests and workflows. Move useful behavior and remove displaced implementations together.

Keep real root handles through store lifetime. Query output must not trigger recursive storage inventory.
Archives and incomplete imports follow the root/maintenance recipes; they never inherit external write authority.
Do not copy these algorithms into local READMEs or preserve a package solely for its historical name.

After code, run minimal pinned compiler/Clippy for changed owners and real consumers. Old lifecycle, pairing,
rotation and receipt fixtures leave with their APIs. Documentation or a prior branch build is not runtime evidence.
