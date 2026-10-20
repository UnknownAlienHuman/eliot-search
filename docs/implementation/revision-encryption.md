# Optional encrypted immutable storage

Encryption is a selected storage profile, not a mandatory property of every local object and not a generic
key-lifecycle programme.

The application supports a plain current-format root. An encrypted Windows-user profile is retained only if
the accepted #598 handoff delivers its concrete object writer/reader and key-use seam. This document does not
design a competing format or modify #598's private branch/worktrees.

## Ownership

```text
immutable-object owner
  object format, identity, write/readback, reader verification

concrete Windows key owner
  exact current-user protected key read/create/readback needed by that profile

redb current state
  root/profile and object references only; never plaintext key or generic secret history
```

Qdrant credentials, provider/restricted-client authentication and old DIRECT compatibility do not share this
key lifecycle.

## Required behavior if the encrypted profile is selected

The accepted profile must define and implement directly:

- one exact versioned authenticated object format;
- actual key/reference and algorithm/nonce sizes;
- nonce uniqueness appropriate to the writer and persisted/recovery model;
- authenticated header/AAD binding the identities needed to prevent cross-object/root/profile substitution;
- exact plaintext/ciphertext bounds;
- atomic immutable publication and exact final readback;
- restart/open behavior and existing-key selection;
- wrong user/key/binding, tamper, truncation and unknown version failures;
- finite key/plaintext ownership and redacted diagnostics;
- no plaintext/key bytes in argv, logs, public config or redb.

The same acquired bytes flow through encryption into the immutable object; the reader verifies/decrypts that
exact object. Do not add another receipt/digest path which can disagree with the actual bytes.

## Key creation and use

- Plain roots perform no key operation.
- A new encrypted root may create exactly the key required by the selected profile only after proving that
  this is explicit initialization with no existing encrypted objects/key.
- Existing encrypted objects require the exact existing key. Missing/mismatched key is a hard failure; never
  generate a replacement.
- A possible credential write is resolved by exact target readback.
- The key is supplied directly to the object codec under the owning operation; no public generic lease,
  purpose catalog or process-wide active-ID selection.

## Rotation and migration

The first working profile has no universal current/candidate/retiring rotation machine. A future key change
may rebuild/export into a new root/profile using an explicit bounded operation after a concrete need exists.
It does not justify per-key usage ledgers, cross-root re-encryption, backup key orchestration or compatibility
with every pre-release format now.

Already encrypted old data is either:

- read through a narrow explicit old-format reader long enough to export/rebuild;
- supported by the accepted #598 current format;
- or refused before writes and left intact.

Ordinary root open must not automatically migrate every retained object.

## Public-main code disposition

Before tests and daemon legacy migration/protection code, public main contains more than 310 KiB across
`search-os-secrets`, `search-os-secrets-windows`, `search-revision-crypto` and daemon `secret_composition`.
Much of it is generic lifecycle, pairing, candidate-selection and compatibility machinery.

After #598 handoff:

1. trace the exact codec/key effects it uses;
2. move only those pieces to the direct immutable-object/Windows owners;
3. remove generic configuration/catalog/lease/rotation/revocation and pairing composition;
4. remove unconsumed candidate/current-selection codecs;
5. delete `search-revision-crypto` if it remains unreferenced;
6. retain an old-format reader only for explicit user data access/export.

## Verification

Run focused integrated cases only after implementation: new plain root, new encrypted root, existing-key
restart, exact object round trip, wrong user/key/profile, tamper/truncation, interrupted object publication,
unknown credential write readback and no secret/plaintext leakage. Do not require universal migration,
rotation, backup, purge or provider qualification matrices for the first useful Search path.
