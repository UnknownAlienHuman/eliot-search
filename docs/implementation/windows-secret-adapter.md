# Windows secret adapter — legacy only

[Local product scope](../architecture/LOCAL_SCOPE.md) selects plain local storage with no application key or
cipher profile. This replaces the previous optional-encryption plan; do not implement its old requirements.

<a id="current-implemented-effects"></a>
<a id="boundary-to-preserve"></a>
## Historical material and removal

[Previous design](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/docs/implementation/windows-secret-adapter.md) is historical, not a work queue.
Use the [security-wiring removal map](../architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring) and
[plain object recipe](../../crates/search-source/search-revision-store/FUNCTIONS.md).
Preserve old user bytes/keys and unpublished #598 work. Refuse unsupported protected roots without writes;
do not silently decrypt, migrate or delete them. Public code removal follows actual caller cutover and minimal
compiler/Clippy checks. No private-work inspection or automatic encrypted-data migration is authorized.
