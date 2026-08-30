# W10 optional depth

This directory is the bounded P16-P18 implementation projection for optional semantic, document and
advanced-scale profiles after an accepted P15 Product Pulse.

- [`W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md`](W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md) — cross-package activation,
  profile identity, migration, removal, rollback and measured-benefit semantics.
- [`manifest.toml`](manifest.toml) — machine ownership and invariant inventory, including the
  candidate-specific `search-eval` evidence owner.
- [`../../qualification/optional-depth/`](../../qualification/optional-depth/README.md) — disabled
  provider/profile templates and candidate-specific G6 evidence.
- [`../../config/w10-optional-depth.toml`](../../config/w10-optional-depth.toml) — locked and bounded
  staging settings; configuration cannot authorize optional depth.
- [`../../swarm/w10-optional-depth.toml`](../../swarm/w10-optional-depth.toml) — one-agent-per-package
  candidate packets and integration prerequisites.
- [`../../crates/search-eval/W10_OPTIONAL_EVALUATION.md`](../../crates/search-eval/W10_OPTIONAL_EVALUATION.md)
  — narrow W10 reentry for paired incremental quality/cost/noninterference/fault/removal evaluation.
- [`../../swarm/stage-readsets.toml`](../../swarm/stage-readsets.toml) — replacement contexts for daemon,
  Qdrant/publication/pins/reclaimer and evaluator packages returning after earlier accepted stages.

No model, tokenizer, runtime, document engine, archive format or scale topology is selected here. Every
candidate remains disabled until one exact accepted P15 report/reviewer receipt, one dedicated ADR,
exact artifact and Windows qualification, pre-registered measured incremental benefit, complete removal
proof and migration/rollback evidence are independently accepted.

The W10 `search-eval` writer receives the exact accepted W9 public API and P15/G5 report/reviewer
receipts. It does not reread W4/W9 implementation packets, select/execute a provider directly, mutate
routes/configuration or self-accept G6.
