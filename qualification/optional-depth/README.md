# P16-P18 optional-depth qualification

This directory is the machine-readable G6 contract for candidate-specific optional profiles after an
accepted P15 Product Pulse.

- [`W10_QUALIFICATION.md`](W10_QUALIFICATION.md) — freeze, execution, review and stop rules.
- [`baseline.toml`](baseline.toml) — shared accepted-P15 and candidate gate template.
- [`model-profile.toml`](model-profile.toml) — disabled model/dense/multivector/rerank profile template.
- [`document-profile.toml`](document-profile.toml) — disabled document materializer profile template.
- [`scale-profile.toml`](scale-profile.toml) — disabled advanced-scale topology template.
- [`probes.toml`](probes.toml) — forty-five disabled probe templates, fifteen per candidate class.
- [`gate-map.toml`](gate-map.toml) — candidate-specific mapping to the five existing G6 evidence IDs.
- [`fixture-owners.toml`](fixture-owners.toml) — semantic fixture owners and accepted-reference slots.

No provider, artifact, runtime, tokenizer, document engine or topology is selected. Static probe templates
remain `DISABLED`; a candidate-specific integration ticket copies the selected profile's templates into
an immutable evidence run after accepted P15, ADR and exact qualification prerequisites exist.
