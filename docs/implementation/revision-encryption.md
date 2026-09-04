# Revision encryption

Large immutable source revisions use a two-level Windows profile:

1. `search-revision-crypto` generates and uses a 256-bit data-encryption key for AES-256-GCM revision envelopes.
2. `search-os-secrets-windows` protects that short key with current-user DPAPI and exact installation/incarnation/purpose entropy.

The revision envelope authenticates the key generation, source-revision binding,
complete residency binding, encryption profile, plaintext SHA-256, plaintext
length, algorithm, version, and nonce. Copying ciphertext into another source,
revision, residency domain, profile, or key generation therefore fails closed.

Only DPAPI-protected key bytes and authenticated revision envelopes may be
persisted. Plaintext keys and decrypted revisions are redacted and zeroized on
drop.

Remaining composition work:

- atomically create or recover the first DPAPI-wrapped key generation;
- bind encrypted envelope persistence to `search-revision-store` intent, readback, and reconciliation;
- rotate keys without replacing immutable source-revision identity;
- admit decrypted bytes only after current owner, policy, residency, and purge-fence revalidation;
- retain restart, tamper, wrong-scope, stale-key, lost-acknowledgement, and purge receipts.
