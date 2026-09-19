# Windows secret adapter

`search-os-secrets-windows` owns the concrete current-user DPAPI effect boundary.
It is a platform adapter, not a second secret lifecycle authority.

Implemented:

- `CryptProtectData` and `CryptUnprotectData` with UI forbidden;
- exact optional-entropy binding supplied by the caller;
- finite short-secret and legacy-revision input/output ceilings;
- redacted and overwrite-on-drop short plaintext ownership;
- one RAII owner for every DPAPI `LocalAlloc` output;
- clear-before-free for admitted native plaintext/ciphertext outputs;
- bounded release without an unbounded clear for an invalid oversized native
  length;
- non-Windows fail-closed stubs;
- Windows round-trip and wrong-scope/entropy tests.

The adapter deliberately does **not** own:

- filesystem or revision-object persistence;
- Credential Manager targets, root-secret creation, RNG, vault locking, or
  migration inventory;
- legacy revision envelope framing or key-derivation domains;
- source identity, catalog policy, access grants, or plaintext admission;
- portable export or cross-user recovery.

`search-os-secrets` owns the pure secret lifecycle and legacy revision
framing/derivation contracts. `eliot-searchd` composes those contracts with the
Credential Manager root-secret lifecycle and translates typed adapter failures
to the existing `DIRECT_DPAPI_*` compatibility reasons.

The accepted large-revision replacement profile remains separate:
AES-256-GCM revision data with a DPAPI-wrapped short DEK. This ownership move
preserves the current legacy `.dpapi` bytes and does not claim that replacement
profile is integrated.
