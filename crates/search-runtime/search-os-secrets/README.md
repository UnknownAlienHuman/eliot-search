# search-os-secrets — retire the universal secret lifecycle

This package mixes a provider-neutral secret catalogue/lifecycle, revision-key candidate/selection scaffolding
and compatibility helpers for a legacy protected revision format. Only concrete selected consumers justify
retained code.

Read the [runtime direction](../README.md), [implementation guide](../../../docs/implementation/README.md),
[storage-profile note](../../../docs/implementation/revision-encryption.md), local [source map](FUNCTIONS.md)
and closed superseded issues #307/#308/#309.

## Revision-specific evidence

At original public-main baseline `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, production source was about
117.9 KiB before tests. The audit found the generic pairing-secret composer constructed by package/daemon tests
rather than normal startup, while legacy revision protection and native key-selection scaffolding still had
daemon references. These are historical caller and size findings, not permanent current-branch facts.

Before deleting or moving this package, repeat an exact search at the implementation head across manifests,
Rust and non-Rust callers, constructors, features, targets, tests and workflows. Preserve #598's active private
work and use only its accepted public handoff; do not infer its final key/object format from this scaffold.

## Concrete ownership

| Consumer | Secret ownership |
|---|---|
| Qdrant | one optional actual API key in Qdrant composition; rebuild the concrete client when settings change |
| trusted child stdio / owner-only local transport | no application pairing secret, grant or per-frame MAC |
| optional restricted client | its selected adapter owns credential, session and revocation |
| encrypted immutable-object profile | one concrete OS-protected key path and authenticated object codec |
| plain immutable-object profile | no encryption key |

There is no shared current/candidate/retiring catalogue, generic purpose lease, operation history or migration
service spanning these domains.

## Existing encrypted or pre-release data

Never silently relabel or rewrite old encrypted objects. A selected application may keep a narrow read-only
old-format reader for explicit access/export, or reject an unsupported root before writes and leave it intact.
New current-format writes use only the accepted immutable-object owner/profile after #598 handoff. Compatibility
code cannot drive normal object identity, key lifecycle or request policy.

## Removal direction

After current callers move, delete the generic catalogue, lifecycle states, leases, rotation/revocation,
operation receipts, configuration fingerprints, provider-neutral backend model, pairing-secret composition,
Qdrant secret-reference indirection and candidate/selection codecs not used by the accepted storage profile.
Delete fixtures that only preserve those APIs. Do not replace them with `secrets-v2`, another registry or a
package-wide provider trait.

Potential retained code is limited to exact metadata/helpers directly consumed by the selected encrypted-object
profile, a narrow old-format reader while real user data requires it, and content-free redacted errors. Move
small retained helpers beside the immutable-object owner or Windows key adapter; delete this package if no
substantial shared pure boundary remains.

## Required properties if an encrypted profile is selected

Key bytes never enter public config, argv, logs, receipts or default errors. The object format owns nonce/key-use
safety, complete authenticated binding, bounded encryption/decryption and truthful corruption/wrong-key failure.
Plaintext and key buffers have finite ownership with best-effort clearing but no secure-erasure claim. No
universal rotation, cross-root migration, backup-key orchestration or purge proof is implied.

After code, run minimal Rust 1.98 check/Clippy for actual retained profile and direct consumers, then focused
fresh/existing/restart/wrong-key/corruption/no-leak cases. Historical catalogue/rotation/grant matrices disappear
with their APIs.
