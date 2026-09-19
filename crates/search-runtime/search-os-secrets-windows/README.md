# search-os-secrets-windows

Concrete Windows current-user DPAPI effect adapter.

The package owns the native `CryptProtectData` / `CryptUnprotectData` boundary,
DPAPI `LocalAlloc` output lifetime, bounded copy-out, and plaintext clear-before-free.
It performs no filesystem, Credential Manager, registry, process, clock, source-catalog,
or policy I/O.

Two bounded public surfaces are available:

- short application secrets use `SecretBytes`, `ProtectionScope`, and
  `ProtectedSecret`;
- legacy DIRECT revision compatibility uses exact inner-envelope bytes and an
  already-derived 32-byte optional-entropy value.

The legacy surface returns only DPAPI ciphertext or untrusted inner-envelope
bytes. The caller still owns the frozen envelope codec, exact revision binding,
plaintext digest validation, persistence, catalog currentness, and source access.

All native output allocations are released exactly once. Decrypted native output
is overwritten before `LocalFree`; an output larger than its admitted bound is
released without an unbounded memory clear. Optional entropy is copied into an
owned buffer and overwritten on drop.

This package is a platform effect adapter, not a second secret lifecycle owner.
The pure lifecycle and legacy framing/derivation contracts remain in
`search-os-secrets`; daemon composition owns Credential Manager root-secret
lifecycle and existing `DIRECT_*` compatibility translation.
