# Windows secret adapter — current legacy inventory

`search-os-secrets-windows` currently owns concrete current-user Windows secret effects. This page records what
exists so the #598 handoff can retain the necessary platform seam. It is **not** the target product architecture
and is not an instruction to extend DIRECT compatibility, generic secret lifecycles or pairing machinery.

The target storage boundary is in [optional encrypted immutable storage](revision-encryption.md). Plain
current-format roots perform no key operation. An encrypted profile exists only through the accepted #598
object/key owner.

## Current implemented effects

The adapter currently provides:

- Credential Manager read/create/readback for the exact 32-byte legacy revision root secret;
- `BCryptGenRandom` creation of a missing root secret;
- the named cross-process revision-vault mutex;
- race-safe create-if-missing and exact post-lock re-read;
- exact credential readback before accepting a new write;
- `CryptProtectData` / `CryptUnprotectData` with UI forbidden and caller-supplied entropy;
- finite input/output ceilings, redacted errors and overwrite-on-drop Rust ownership;
- RAII ownership for Credential Manager and DPAPI allocations;
- clear-before-free for admitted native plaintext/ciphertext outputs;
- non-Windows fail-closed stubs and focused Windows round-trip/wrong-scope tests.

The test-only `test-credential-cleanup` feature additionally owns exact-target `CredDeleteW` under the same
mutex with readback-verified absence. Production builds do not expose that cleanup path.

## Boundary to preserve

Keep only actual Windows key effects required by the selected immutable-object profile. The adapter must not
own filesystem/object persistence, source identity, root membership, generic key selection, provider/Qdrant
credentials, portable export or cross-user recovery.

Current `search-os-secrets`, daemon `secret_composition` and legacy revision framing add broader lifecycle,
candidate, compatibility and pairing machinery. After the accepted #598 handoff:

1. trace the exact key read/create/protect/unprotect calls used by the current object codec;
2. move only those effects into the concrete object/Windows owners;
3. remove generic catalogue/lease/rotation/revocation and DIRECT compatibility composition;
4. retain an old-format reader only for explicit user-data export when required;
5. never generate a replacement key for existing encrypted objects.

No change to #598 private files or worktrees is authorized by this document. Current behavior and target
disposition remain separate until the public handoff is accepted and integrated.
