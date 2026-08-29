# `[source_reader]` configuration contract

- **Owner:** `search-safe-reader`
- **Earliest wave:** W2
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `max_file_bytes` | `u64` | `16777216; cannot exceed admission ceiling` | `file` | `APPLY_LIVE decrease / RESTART increase` |
| `stable_read_attempts` | `u8` | `3; 1..8` | `file` | `APPLY_LIVE` |
| `retry_delay_ms` | `u64` | `25; 0..1000` | `file` | `APPLY_LIVE` |
| `git_network_allowed` | `bool` | `false; fixed` | `none` | `REJECT` |
| `execute_hooks` | `bool` | `false; fixed` | `none` | `REJECT` |
| `execute_filters` | `bool` | `false; fixed` | `none` | `REJECT` |

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

- network, hooks and filters remain disabled
- final-handle root containment cannot be configured away
- bytes and absolute paths stay out of default diagnostics

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
