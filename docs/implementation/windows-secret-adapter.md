# Windows secret adapter

`search-os-secrets-windows` implements the concrete current-user DPAPI boundary.
It is a platform adapter, not a second secret lifecycle authority.

Implemented:

- `CryptProtectData` and `CryptUnprotectData` with `CRYPTPROTECT_UI_FORBIDDEN`;
- mandatory length-prefixed scope entropy for user, installation, incarnation, and purpose;
- finite plaintext, entropy, and protected-blob limits;
- exact Windows error classification;
- redacted plaintext and overwrite-before-free for Rust- and DPAPI-owned buffers;
- fail-closed non-Windows behavior.

Still required before production readiness:

- compose the adapter with the daemon and durable secret lifecycle;
- persist only protected bytes plus exact version and binding metadata;
- qualify service-account/profile behavior on the target Windows installation;
- retain rotation, restart, wrong-user, wrong-scope, deletion, and recovery receipts.
