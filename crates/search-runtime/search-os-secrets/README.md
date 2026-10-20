# search-os-secrets — retire the universal secret lifecycle

This package currently mixes three different concerns:

1. a generic provider-neutral secret catalog/lifecycle;
2. metadata/codecs for a planned revision-DEK selection system;
3. pure compatibility framing/derivation for the legacy DIRECT protected revision format.

Only concrete selected consumers justify retained code. Read the [runtime direction](../README.md),
[implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md) and closed
superseded issues [#307](https://github.com/UnknownAlienHuman/eliot-search/issues/307),
[#308](https://github.com/UnknownAlienHuman/eliot-search/issues/308) and
[#309](https://github.com/UnknownAlienHuman/eliot-search/issues/309).

## Verified public-main inventory

Production source is about **117.9 KiB before tests**:

```text
configuration.rs                    13.1 KiB
legacy_revision_protection.rs        0.4 KiB
legacy_revision_protection/codec.rs  8.7 KiB
legacy_revision_protection/model.rs  7.8 KiB
lib.rs                               1.0 KiB
lifecycle.rs                        46.6 KiB
revision_candidate.rs               24.2 KiB
revision_selection.rs               16.2 KiB
```

The generic `SecretCatalog` is composed by daemon `PairingSecretComposer`; public-main construction of that
composer was found in package/daemon tests rather than normal application startup. Trusted child stdio no
longer needs an application pairing secret, grant or per-frame MAC. Qdrant uses its own optional actual key.

Legacy revision protection and native DEK-selection scaffolding have real daemon references. Preserve their
current behavior until #598 produces an accepted handoff, then keep only the selected current-format storage
profile and any explicitly retained old-format reader.

## Target ownership

### Ordinary local application

No general secret subsystem exists. Concrete owners handle concrete material:

| Consumer | Secret ownership |
|---|---|
| Qdrant | optional actual API key in Qdrant composition; rebuild client on rotation |
| trusted child stdio / owner-only pipe | no application pairing secret, product grant or frame MAC |
| optional restricted client | selected authentication adapter owns credential/session/revocation |
| immutable encrypted object profile | one concrete OS-protected key owner and authenticated object codec |
| plain immutable object profile | no encryption key |

There is no shared current/candidate/retiring catalog, generic purpose lease, operation history or migration
service across these domains.

### Existing encrypted/pre-release data

Do not silently relabel or rewrite old encrypted objects. The chosen application may:

- keep a narrow read-only old-format reader while data is explicitly exported/rebuilt;
- reject the unsupported root before writes and leave it intact;
- use the accepted #598 current-format writer/reader after its handoff.

Compatibility code is not allowed to drive new object identity, key lifecycle or normal request policy.

## Removal direction

Delete with callers:

- `SecretCatalog`, `SecretRecord`, current/candidate/retiring/tombstone lifecycle and limits;
- create/rotate/delete/recover mutation effects and content-free receipts;
- generic `SecretLease`, TTL/grace/application-purpose selection and operation history;
- `[secrets]` descriptor/fingerprint/delta/application API;
- Qdrant secret-reference field and generic backend selection;
- pairing-secret catalog/composer/rotation/revocation/vault path;
- general provider/backup/migration consumer abstractions;
- revision-DEK candidate/current selection codecs which the accepted #598 result does not use;
- tests/fixtures/goldens protecting those APIs.

Do not replace this with `secrets-v2`, another registry or a package-wide provider trait.

## Potential retained code

Only after #598's accepted handoff:

- minimal immutable key/reference metadata actually stored by the current encrypted profile;
- exact authenticated object header/AAD helpers if that profile directly uses them;
- a narrow old DIRECT envelope reader if user data still needs explicit access/export;
- content-free errors and redacted secret owners.

Move retained types/helpers beside the immutable-object writer/reader or Windows key adapter. Delete this
package if no substantial shared pure code remains.

## Required properties of a retained encrypted profile

- key bytes never enter public config, argv, logs, receipts or default errors;
- exact object format owns nonce uniqueness appropriate to that format;
- header/AAD binds the identity needed to prevent cross-object substitution;
- encryption/decryption reads the same bounded bytes used by object persistence;
- corruption/wrong-key/wrong-binding is a truthful read failure;
- plaintext/key buffers have finite ownership/lifetime and best-effort clearing without secure-erasure claims;
- no universal rotation, cross-root migration, backup-key orchestration or purge proof is implied.

#598 remains active private work. This audit assigns no competing writer and does not infer its final object or
key format from public-main scaffolding.
