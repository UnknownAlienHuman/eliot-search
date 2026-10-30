# Application encryption — not selected

[Local product scope](../architecture/LOCAL_SCOPE.md) selects plain local storage with no application key or
cipher profile. This replaces the previous optional-encryption plan; do not implement its old requirements.

<a id="ownership"></a>
<a id="required-behavior-if-the-encrypted-profile-is-selected"></a>
<a id="key-creation-and-use"></a>
<a id="rotation-and-migration"></a>
<a id="historical-public-main-disposition-inventory"></a>
<a id="verification"></a>
## Historical material and removal

[Previous design](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/docs/implementation/revision-encryption.md) is historical, not a work queue.
Use the [security-wiring removal map](../architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring) and
[plain object recipe](../../crates/search-source/search-revision-store/FUNCTIONS.md).
Preserve old user bytes/keys and unpublished #598 work. Refuse unsupported protected roots without writes;
do not silently decrypt, migrate or delete them. Public code removal follows actual caller cutover and minimal
compiler/Clippy checks. No private-work inspection or automatic encrypted-data migration is authorized.
