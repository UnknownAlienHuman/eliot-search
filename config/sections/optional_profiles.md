# `[optional_profiles]` configuration contract

- **Owner:** `eliot-searchd`
- **Earliest wave:** W10
- **Section minimum:** `GATE_REQUIRED`
- **Secret policy:** `opaque_refs_only`
- **Canonical sources:** `docs/config/CONFIGURATION_1.0.md`, `docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md`

This packet is the bounded section-owner contract. It does not authorize optional implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default | Override | Minimum action |
|---|---|---|---|---|
| `semantic` | `bool` | `false` | `file` | `GATE_REQUIRED`; generation/rebuild for dense/multivector |
| `document` | `bool` | `false` | `file` | `GATE_REQUIRED + NEW_COLLECTION_GENERATION + REBUILD_PROJECTION` |
| `advanced_scale` | `bool` | `false` | `file` | `GATE_REQUIRED + SCALE_MIGRATION` |
| `model_provider_profile` | `ProfileId | null` | `null` | `file` | candidate-specific gate/qualification |
| `document_provider_profile` | `ProfileId | null` | `null` | `file` | candidate-specific gate/qualification |
| `scale_profile` | `ProfileId | null` | `null` | `file` | measured-bottleneck gate/qualification |

Profile IDs are opaque references to accepted candidate-specific receipts. Paths, versions, model names,
topology values and credentials are not accepted as substitutes.

## Required section API

```text
section_descriptor() -> ConfigSectionDescriptor
compiled_defaults() -> ConfigSectionInput
validate_section(input, platform, accepted_capabilities) -> Result<ValidatedSection, ConfigError>
section_digest(validated) -> Blake3Digest32
plan_section_change(old, new) -> Result<SectionReloadDecision, ConfigError>
apply_live_change(old, new, context) -> Result<ConfigApplyReceipt, ConfigError>
```

`apply_live_change` cannot activate, change or remove an optional profile. It may return only `NOOP` for
an equal effective section. All real transitions are daemon-orchestrated composite plans.

## Gate prerequisites

Every requested candidate requires:

- exact independently accepted P15 Product Pulse receipt;
- dedicated candidate ADR;
- compiled candidate feature;
- exact provider/profile/artifact/runtime/Windows/license qualification;
- measured material benefit receipt;
- current binding authorization;
- removal/fallback receipt;
- migration/rollback receipt or reviewed rerank-only no-persistent-state receipt.

Configuration cannot create or weaken these receipts.

## Change semantics

- `semantic=false -> true`: G6 gate and worker start; rerank-only has no persistent vectors, while dense/
  multivector additionally require new collection generation and projection rebuild.
- `document=false -> true`: G6 gate, document worker, new representation/projection and collection
  generation.
- `advanced_scale=false -> true`: G6 gate after measured one-shard bottleneck and full P18 migration.
- changing any active profile ref: drain/remove old candidate and qualify a new candidate; no in-place
  reinterpretation.
- disabling: restore accepted P15 handler/profile/route/config before worker stop, pin drain and exact
  optional reclaim; no silent live toggle.

Required obligations form a set; security, worker, generation/rebuild, route drain/reclaim and gate steps
may coexist and cannot be collapsed to a weaker scalar.

## Invariants

- all optional flags false and refs absent by default;
- no model/document/scale provider selected in baseline;
- configuration/feature/worker readiness never authorizes serving;
- network, auto-download/update, training/learning and persistent content cache are forbidden;
- provider output grants no source/client/exact-proof authority;
- active collection schema/topology is never mutated in place;
- capability snapshot is coherent with handler/worker/profile/route/config state;
- optional failure leaves accepted P15 baseline available with explicit degradation;
- removal proves worker/cache/route cleanup and P15 regression;
- secure erase is not claimed without evidence.

## Required tests

- defaults validate and produce stable section digest;
- unknown keys/types and plaintext secrets fail closed;
- profile ref without exact accepted receipt is rejected;
- config alone, feature alone and worker readiness alone cannot activate;
- rerank-only vs dense/multivector action classification;
- document and scale always require migration/rebuild obligations;
- active profile change cannot apply live;
- failed partial activation preserves previous baseline fingerprint;
- disabling requires baseline restore/removal receipt;
- equal inputs produce equal digest/plan;
- redacted diagnostics contain no artifact path, model/document identity, topology details, content or secret.
