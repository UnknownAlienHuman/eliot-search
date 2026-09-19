# Windows secret adapter

`search-os-secrets-windows` implements the concrete current-user Windows secret
effect boundary. It is a platform adapter, not a source, catalog, filesystem, or
access-policy authority.

Implemented:

- `CredReadW`, `CredWriteW`, and `CredFree` ownership for the legacy revision
  namespace root secret;
- exact lower-case namespace credential target and local-machine persistence;
- `BCryptGenRandom` generation of the fixed 32-byte root secret;
- named cross-process vault mutex and bounded retry/readback protocol;
- post-lock credential re-read so a concurrent winner is adopted rather than
  overwritten;
- exact readback verification before a newly written key is returned;
- `CryptProtectData` and `CryptUnprotectData` with
  `CRYPTPROTECT_UI_FORBIDDEN`;
- mandatory optional entropy and finite plaintext/protected-blob limits;
- typed Windows error classification;
- redacted secret owners and overwrite-before-free for Rust-, Credential
  Manager-, and DPAPI-owned secret buffers;
- fail-closed non-Windows behavior.

Daemon composition remains responsible for scanning the admitted revision root
and choosing one explicit requirement:

- `CreateIfMissing` only when no protected revision object exists;
- `RequireExisting` when any protected revision object exists.

The adapter receives no filesystem path, source identity, catalog record, access
policy, revision body, or persistence authority. The pure legacy envelope and
key-derivation contracts remain in `search-os-secrets`; concrete SHA-256 remains
in daemon composition.

Still required before production readiness:

- move or replace the remaining test-only daemon credential deletion helper;
- qualify Credential Manager and DPAPI behavior under the target Windows service
  account/profile;
- execute concurrent first-open, restart, wrong-user, missing-key, tampered
  credential, deletion, and recovery fixtures;
- complete T18 purpose-bound secret leases and pairing independently of legacy
  revision compatibility;
- retain exact execution and independent-review receipts.
