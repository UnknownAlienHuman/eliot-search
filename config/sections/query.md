# `[query]` configuration contract

- **Owner:** `search-query-planner`
- **Earliest wave:** W4
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `default_budget_class` | `ProfileId` | `interactive-default-v1` | `file, CLI` | `APPLY_LIVE` |
| `max_scoring_legs` | `u32` | `16; finite` | `file` | `APPLY_LIVE` |
| `max_prefetch_candidates_per_leg` | `u32` | `256; finite` | `file` | `APPLY_LIVE` |
| `max_validated_candidates` | `u32` | `64; finite` | `file` | `APPLY_LIVE` |
| `max_materialized_result_bytes` | `u64` | `1048576; finite` | `file` | `APPLY_LIVE` |
| `fusion_profile_id` | `FusionProfileId` | `accepted weighted-RRF profile` | `file` | `REBUILD_PROJECTION or profile receipt` |

## Required section API

```text
section_descriptor() -> ConfigSectionDescriptor
compiled_defaults() -> ConfigSectionInput
validate_section(input, platform, accepted_capabilities) -> Result<ValidatedSection, ConfigError>
section_digest(validated) -> Blake3Digest32
plan_section_change(old, new) -> Result<SectionReloadDecision, ConfigError>
apply_live_change(old, new, context) -> Result<ConfigApplyReceipt, ConfigError>
```

`apply_live_change` is legal only for fields whose final action is `APPLY_LIVE`. Restart, security
barrier, rebuild, collection-generation and gate actions are executed by daemon orchestration.

## Invariants

- client grants may narrow but never exceed ceilings
- zero never means unlimited
- fusion changes cannot silently alter an active profile

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
