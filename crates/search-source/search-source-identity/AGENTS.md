# Working on search-source-identity

Read [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md), the
[source registry map](../search-source-registry/FUNCTIONS.md) and the
[implementation guide](../../../docs/implementation/README.md).

Do not preserve the 149 KiB identity/history/lineage framework. Keep only the current source matching needed
by real inventory/ingestion:

- current root-relative locator under the root's actual lookup behavior;
- optional exact native physical key from the held final handle;
- one persisted `SourceId` and current locator/physical key;
- explicit same-source / proven-rename / new / conflict result.

Content digest is revision identity, not source identity. Path text or content equality never proves two
sources equal. Missing physical evidence remains explicit.

Remove binding interval history, owner generations, batch/current/recorded wrappers, repository lineage,
workspace/fork/mirror classification, redaction framework, canonical evidence receipts and legacy/Git digest
profiles with their callers/fixtures. Git uses the exact repository object-format OID plus selected tree/path.

If the remaining matcher/NTFS key fits naturally beside source reconcile/native reading, move it and delete
this crate; do not leave a forwarding facade.

Preserve #598's active private source work. After code, run minimal Rust 1.98 compiler/Clippy for changed
source/native/registry consumers and focused rename/replacement/hardlink/restart cases.
