# Windows secret adapter

`search-os-secrets-windows` owns the concrete current-user Windows secret-effect
boundary. It is a platform adapter, not a source/catalog authority or a second
persistence owner.

Implemented:

- Credential Manager read/create/readback for the exact 32-byte legacy revision
  root secret;
- `BCryptGenRandom` creation of a missing root secret;
- the named cross-process revision-vault mutex;
- race-safe create-if-missing with an exact post-lock re-read;
- exact readback before a newly written credential is accepted;
- `CryptProtectData` and `CryptUnprotectData` with UI forbidden;
- exact optional-entropy binding supplied by the caller;
- finite short-secret and legacy-revision input/output ceilings;
- redacted and overwrite-on-drop Rust secret ownership;
- one RAII owner for every Credential Manager and DPAPI allocation;
- clear-before-free for admitted native plaintext/ciphertext outputs;
- bounded release without an unbounded clear for an invalid oversized native
  length;
- non-Windows fail-closed stubs;
- Windows round-trip and wrong-scope/entropy tests.

The optional `test-credential-cleanup` feature additionally owns exact-target
`CredDeleteW`, serialization under the same vault mutex, and readback-verified
absence for native harness cleanup. The feature is enabled only by daemon test
builds; production daemon builds do not expose or compile this deletion path.

The adapter deliberately does **not** own:

- filesystem or revision-object persistence;
- protected-object discovery or the decision to create a missing key;
- legacy revision envelope framing or key-derivation domains;
- source identity, catalog policy, access grants, or plaintext admission;
- portable export or cross-user recovery;
- parsing a daemon data root's namespace file.

`search-os-secrets` owns the pure secret lifecycle and legacy revision
framing/derivation contracts. `eliot-searchd` composes those contracts with
qualified protected-object inventory and translates typed adapter failures to
the existing `DIRECT_*` compatibility reasons.

The accepted large-revision replacement profile remains separate:
AES-256-GCM revision data with a DPAPI-wrapped short DEK. These ownership moves
preserve the current legacy `.dpapi` bytes and do not claim that replacement
profile is integrated.
