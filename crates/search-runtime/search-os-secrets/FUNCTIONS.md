# search-os-secrets — source audit and deletion map

The package is not the application secret authority. It is a public-main mixture of a generic lifecycle,
revision-key planning metadata and legacy DIRECT compatibility. The current architecture selects concrete
consumer-specific ownership.

## Current source and caller reality

| Source/API | Actual behavior / caller reality |
|---|---|
| `configuration.rs` | Owns a generic `[secrets]` section with backend, lease TTL, rotation grace and Qdrant reference plus descriptor/digest/reload semantics. The selected consumers no longer share these fields. |
| `lifecycle.rs` | About 46.6 KiB. Implements `SecretCatalog`, records, ciphertext digests, create/rotation/delete/recovery effects, leases, tombstones, operations and finite catalogue limits. The daemon pairing composer is its main non-test wrapper; composer construction was found in tests, not normal startup. |
| `revision_candidate.rs` | About 24.2 KiB of canonical binding/intent/candidate metadata for DPAPI-wrapped revision DEKs. It proves metadata equality, not key existence/use or object encryption. |
| `revision_selection.rs` | About 16.2 KiB of policy/application intent/current selection codecs and multiple revision/fingerprint/epoch axes. It is connected to owner-composition scaffolding, but the accepted current encrypted-object writer remains #598. |
| `legacy_revision_protection/**` | About 16.8 KiB of pure old DIRECT inner/outer envelope framing, key binding and entropy derivation. Actual legacy revision protection uses it. |
| generic `SecretCatalog` consumers | `PairingSecretComposer` + test/process fixtures. Qdrant and current local protocol no longer require it. |
| `search-revision-crypto` | Separate 19.3 KiB AES-GCM package with no public-main Rust caller/incoming dependency found. It must not dictate this package's permanent lifecycle. |

## Current architecture decisions

- #307 universal secret lifecycle is closed `not_planned`.
- #308 general Windows secret backend is closed `not_planned`.
- #309 universal revision-AEAD lifecycle/migration is closed `not_planned`.
- #280 removes mandatory local pairing/grant/MAC.
- #263 owns the optional actual Qdrant key directly.
- #274 owns a selected restricted-client credential/session directly.
- #598 remains the only active current-format source/encrypted-object handoff and is not inspected or
  duplicated by this audit.

## Replace generic configuration

Do not preserve:

```text
backend = windows_user_bound
lease_ttl_ms
rotation_grace_ms
qdrant_api_secret_ref
section descriptor/digest/reload receipt
```

Instead:

- root initialization selects a storage profile (`plain` or the accepted encrypted profile);
- Qdrant configuration owns its optional actual API key;
- restricted-client configuration owns its concrete authentication reference;
- static changes restart/reopen their concrete owner;
- no live generic secret reconfiguration exists.

## Delete generic lifecycle

Remove `SecretCatalog` and all public lifecycle records/operations after callers switch:

```text
EncryptedPayload / SecretRecord / SecretRecordState
SecretMutationEffect / create / prepare_rotation / confirm_rotation
rotate / delete / recover_operation
SecretLease and consumer/purpose/TTL authority
catalog operation IDs, tombstones, quarantine and historical receipts
```

Concrete consumers do not submit encrypted payload/digest/operation records to a central catalog. They load or
hold their own material and perform their own exact effect/readback where required.

The daemon `secret_composition` production files are about **40.9 KiB before tests** and implement pairing
binding, composer, receipts, rotation, revocation and vault. Since trusted local protocol no longer uses that
pairing key and public construction is test-only, remove the module and its protocol/CLI credential callers
under #280/#332 rather than connecting it.

## Encrypted storage handoff

Do not redesign #598. After its accepted handoff, inspect the exact code/API it delivers and retain only:

```text
selected root storage profile
one concrete key owner
exact current-format authenticated object codec
one writer/readback and one reader/verification path
```

Move any required pure metadata beside the object/key owner. If #598 does not consume
`RevisionDekCandidate`, selection records or `search-revision-crypto`, delete those APIs/packages and their
control/Windows/fixture callers.

No generic rotation is required for the first current-format encrypted profile. A later key change may rebuild
objects into a new root/profile or add a narrow measured operation. It does not justify current/candidate/
retiring roles, durable usage ledgers or cross-root re-encryption now.

## Legacy DIRECT reader

While actual user data still needs it, keep a narrow old-format reader composed from:

- exact legacy envelope parse/validation;
- current-user Credential Manager root-secret read;
- DPAPI unprotect;
- actual content digest/length verification.

Do not let this reader create new legacy objects, migrate automatically on open, select the current-format key
or impose old framing on #330/#598. Remove it after explicit export/rebuild or when unsupported old roots are
rejected by product policy.

## Failure semantics

- missing/wrong encrypted key/object binding: read failure; never generate a replacement for existing data;
- malformed/corrupt authenticated object: corruption failure; never fall back to plaintext/path bytes;
- no key for a newly selected encrypted root: initialization may create exactly the key required by the
  selected current profile, with exact platform readback;
- uncertain platform credential write: inspect that concrete credential reference; do not add a generic
  operation ledger;
- deletion means future application resolution is unavailable, not secure erasure;
- plain profile performs no secret work.

## Removal checklist

After #598 handoff and current caller cutover:

- delete generic configuration/lifecycle modules and daemon pairing `secret_composition`;
- delete Qdrant/provider/backup/migration secret abstractions;
- delete unconsumed revision candidate/selection codecs;
- delete legacy write/migration paths and old lifecycle fixtures;
- delete `search-revision-crypto` if it still has no current writer/reader;
- move the small retained encrypted-profile helpers to direct owners and delete this package if empty.

Focused checks belong to the selected profile: fresh root initialization, existing key readback, exact object
round trip/restart, wrong user/key/binding, corruption/truncation, no plaintext/log leakage and honest
unsupported old-root behavior. Historical catalog/rotation/grant matrices do not block implementation.
