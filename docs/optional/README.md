# W10 optional depth

This directory is the bounded P16-P18 implementation projection for optional semantic, document and
advanced-scale profiles after an accepted P15 Product Pulse.

- [`W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md`](W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md) — cross-package activation,
  profile identity, migration, removal and rollback semantics.
- [`manifest.toml`](manifest.toml) — machine ownership and invariant inventory.
- [`../../qualification/optional-depth/`](../../qualification/optional-depth/README.md) — disabled
  provider/profile templates and candidate-specific G6 evidence.
- [`../../config/w10-optional-depth.toml`](../../config/w10-optional-depth.toml) — locked and bounded
  staging settings; configuration cannot authorize optional depth.
- [`../../swarm/w10-optional-depth.toml`](../../swarm/w10-optional-depth.toml) — one-agent-per-package
  read sets and integration prerequisites.

No model, tokenizer, runtime, document engine, archive format or scale topology is selected here. Every
candidate remains disabled until one exact accepted P15 receipt, one dedicated ADR, exact artifact and
Windows qualification, measured incremental benefit, removal proof and migration/rollback evidence are
independently accepted.
