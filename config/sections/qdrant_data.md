# `[qdrant_data]` configuration contract

- **Owner:** `search-qdrant-bridge`
- **Earliest wave:** W3
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `request_timeout_ms` | `u64` | `5000; bounded` | `file` | `APPLY_LIVE` |
| `upsert_batch_points` | `u32` | `256; byte-bounded` | `file` | `APPLY_LIVE` |
| `delete_batch_points` | `u32` | `512; byte-bounded` | `file` | `APPLY_LIVE` |
| `readback_batch_points` | `u32` | `256; byte-bounded` | `file` | `APPLY_LIVE` |
| `strict_mode` | `bool` | `true; fixed after admission` | `none` | `REJECT` |
| `wait_for_mutations` | `bool` | `true; fixed correctness path` | `none` | `REJECT` |
| `strong_ordering` | `bool` | `true; fixed correctness path` | `none` | `REJECT` |

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

- strict mode blocks unindexed retrieve/update filters
- publication mutations use wait=true, strong ordering and exact readback
- schema/capability changes require a new collection generation

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
