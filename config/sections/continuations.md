# `[continuations]` configuration contract

- **Owner:** `search-continuation`
- **Earliest wave:** W4
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `ephemeral_ttl_ms` | `u64` | `300000; finite` | `file` | `APPLY_LIVE restrictive` |
| `max_per_binding` | `u32` | `64; finite` | `file` | `APPLY_LIVE` |
| `max_pinned_candidates` | `u32` | `512; finite` | `file` | `APPLY_LIVE` |
| `max_pin_ttl_ms` | `u64` | `300000; finite` | `file` | `APPLY_LIVE restrictive` |
| `durable_ordinary_queries` | `bool` | `false; fixed` | `none` | `REJECT` |

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

- raw Qdrant cursors/scores are never public
- ephemeral state dies on restart
- durable replan checkpoints own no process-local pin

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
