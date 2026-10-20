# Runtime family

Organizational directory, not a Cargo package. Runtime code exists only for a real process/OS/storage effect
which cannot live more simply in its direct owner.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and
[storage-profile note](../../docs/implementation/revision-encryption.md).

## Current direction

- [`search-runtime-owner/`](search-runtime-owner/) keeps one actual process/data-root ownership guard while
  the root is open. Remove its lease/receipt ceremony; retain real lock, custody and bounded drain/release.
- [`search-retention/`](search-retention/) is a removal target. Current/pending/pinned references and finite
  cleanup obligations replace generic leases, graph sweep, purge, restore and legal-hold machinery.
- [`search-provider-transport-windows/`](search-provider-transport-windows/) survives only if an owner-only
  persistent Windows pipe remains selected after child-stdio integration. Otherwise delete it.
- [`search-os-secrets/`](search-os-secrets/) and
  [`search-os-secrets-windows/`](search-os-secrets-windows/) currently mix selected legacy/encrypted-storage
  primitives with an unconnected universal secret lifecycle. Keep only direct consumers.
- [`search-revision-crypto/`](search-revision-crypto/) has no incoming public-main Rust dependency/caller.
  Delete it unless the accepted #598 handoff selects it directly as the current encrypted object codec.

## Secret/storage rule

Fresh current-format roots select one storage profile at initialization:

```text
plain local immutable objects
or
selected Windows-user encrypted immutable objects
```

The profile is owned by the immutable-object writer/reader. It is not a generic service for Qdrant,
provider pairing, backup, migration and arbitrary future consumers.

- Qdrant uses one optional actual credential in its own composition.
- trusted child stdio / owner-only local query needs no pairing secret or per-frame MAC.
- an optional restricted-client adapter owns its concrete credential/session.
- an encrypted storage profile owns one concrete OS-protected key path and authenticated object format.
- unsupported pre-release roots are refused before writes; no universal cross-root key migration.

Preserve #598's active private source work and consume only its accepted public handoff. Do not create a
competing object/key writer or infer its final format from public-main scaffolding.

## Removal scale already confirmed

Before daemon revision-protection/migration code and tests:

| Area | Gross production source |
|---|---:|
| `search-os-secrets` | ~117.9 KiB |
| `search-os-secrets-windows` | ~133.1 KiB, plus an optional ~7.5 KiB test-cleanup surface |
| `search-revision-crypto` | ~19.3 KiB |
| daemon `secret_composition` | ~40.9 KiB |

This is more than **310 KiB** around several overlapping secret/key designs. It is inventory, not a promised
net deletion. The retained encrypted profile may keep a small portion after #598 handoff; generic lifecycle,
pairing, migration and uncalled crypto paths do not survive merely to preserve old APIs.

A runtime package does not earn permanence through a security-sounding name, receipt or future consumer.
Keep concrete effects and truthful failures; delete hypothetical authority and compatibility layers.
