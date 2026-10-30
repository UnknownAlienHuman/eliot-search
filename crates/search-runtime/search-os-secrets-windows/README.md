# search-os-secrets-windows — legacy, not selected

The local product uses plain storage and owned-child communication. Do not add key creation, encryption,
rotation, pairing or a replacement secret service here. See [local scope](../../../docs/architecture/LOCAL_SCOPE.md).

<a id="verified-public-main-inventory"></a>
<a id="target"></a>
<a id="remove-with-callers"></a>
<a id="legacy-old-format-support"></a>
<a id="required-windows-semantics"></a>
<a id="598-remains-active-private-work-do-not-modify-or-duplicate-its-unpublished-writerkey-use-design-this-audit"></a>
## Removal boundary

This documentation does not delete the existing package. Follow the
[caller cutover](../../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring): move actual CLI/daemon/
store consumers to the plain path, then remove unused source, Cargo entries and exclusive fixtures together.
Keep shared OS file/lock behavior where used. Do not delete credentials or old user data as cleanup.

[Prior inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-runtime/search-os-secrets-windows/README.md) records the previous design,
not a future integration requirement. Preserve unpublished #598 work without waiting for or importing it.
After source changes run minimal compiler/Clippy for actual consumers, including Windows targets where affected.
