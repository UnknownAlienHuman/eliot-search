# search-os-secrets — legacy, not selected

The local product uses plain storage and owned-child communication. Do not add key creation, encryption,
rotation, pairing or a replacement secret service here. See [local scope](../../../docs/architecture/LOCAL_SCOPE.md).

<a id="revision-specific-evidence"></a>
<a id="concrete-ownership"></a>
<a id="existing-encrypted-or-pre-release-data"></a>
<a id="removal-direction"></a>
<a id="required-properties-if-an-encrypted-profile-is-selected"></a>
## Removal boundary

This documentation does not delete the existing package. Follow the
[caller cutover](../../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring): move actual CLI/daemon/
store consumers to the plain path, then remove unused source, Cargo entries and exclusive fixtures together.
Keep shared OS file/lock behavior where used. Do not delete credentials or old user data as cleanup.

[Prior inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-runtime/search-os-secrets/README.md) records the previous design,
not a future integration requirement. Preserve unpublished #598 work without waiting for or importing it.
After source changes run minimal compiler/Clippy for actual consumers, including Windows targets where affected.
