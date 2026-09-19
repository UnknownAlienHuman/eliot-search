# search-os-secrets-windows

Concrete Windows current-user secret effect adapter.

The package owns the native boundaries for:

- Credential Manager read/create/readback of the 32-byte legacy revision root
  secret;
- `BCryptGenRandom` generation of a missing root secret;
- the named cross-process revision-vault mutex;
- `CryptProtectData` / `CryptUnprotectData`;
- Credential Manager and DPAPI native allocation lifetimes;
- bounded copy-out and clear-before-free.

It performs no filesystem, registry, source-catalog, process, clock, or policy
I/O. Daemon composition scans the admitted revision root and passes exactly one
requirement: create is allowed only when no protected revision object exists;
otherwise the original credential is mandatory.

The create path rechecks Credential Manager after acquiring the named mutex. A
concurrent process that published a key after the initial read therefore wins;
a stale generated candidate never overwrites that key. Every successful write
must read back the exact same 32 bytes before the secret is returned.

Three bounded public surfaces are available:

- legacy revision root-secret operations use `LegacyRevisionRootSecret`,
  `LegacyRevisionRootSecretRequirement`, and the two load functions;
- short application secrets use `SecretBytes`, `ProtectionScope`, and
  `ProtectedSecret`;
- legacy DIRECT revision compatibility uses exact inner-envelope bytes and an
  already-derived 32-byte optional-entropy value.

Secret owners are non-clone, redact `Debug`, and overwrite their Rust buffers on
drop. Native Credential Manager and decrypted DPAPI allocations are cleared
before release when their reported length is within the admitted bound.
Oversized invalid native output is released without an unbounded memory clear.

The pure envelope, root-secret shape, and key-derivation contracts remain in
`search-os-secrets`. The daemon retains source/catalog authority, protected-object
inventory, concrete SHA-256, persistence, and translation to historical
`DIRECT_*` compatibility reasons.
