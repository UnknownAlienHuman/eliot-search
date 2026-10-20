# search-revision-crypto — unconnected candidate, not a permanent subsystem

This 19.3 KiB AES-256-GCM package has no incoming public-main Rust dependency/caller. Source comments in the
revision store mirror its proposed envelope, but no checked-in writer/reader invokes it.

Do not treat source presence, a workspace member or planned consumers as a delivered encrypted storage
profile. Universal key lifecycle/migration issue #309 is closed `not_planned`.

## Disposition

Preserve #598's active private work and wait for its accepted public handoff.

- If that handoff directly uses this codec for the selected current encrypted immutable-object format,
  simplify it with the actual writer/reader and move it beside that object boundary if a separate crate is
  unnecessary.
- If it uses another accepted object/key format, delete this package, its mirrored revision-store binding
  assumptions and documentation.
- Do not keep it for hypothetical rotation, migration, backup or future providers.

## Requirements if retained

Only the concrete current format matters:

- accepted algorithm/key/nonce size and one exact versioned envelope;
- nonce uniqueness policy owned by the actual writer;
- authenticated header/AAD binding the identities required to prevent substitution;
- exact finite plaintext/ciphertext bounds;
- wrong key/binding, tamper, truncation and unknown version fail closed;
- key/plaintext ownership is finite and redacted with best-effort clearing, not secure-erasure claims;
- same bytes flow from acquisition to encryption to immutable persistence/readback;
- decryption returns bytes only to the verified object reader;
- no current/candidate/retiring key roles, generic references, usage ledgers or cross-root re-encryption unless
  a later measured product requirement explicitly adds them.

A plain fresh-root profile does not depend on this package. An old encrypted root may use a narrow explicit
legacy reader/export path and does not force this format onto new objects.

After the #598 handoff, run minimal compiler/Clippy and exact object round-trip/restart/tamper/wrong-binding/
nonce-policy tests through the real immutable-object owner. Package-only crypto fixtures are not product
integration.
