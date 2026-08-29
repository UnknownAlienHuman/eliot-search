# `[observability]` configuration contract

- **Owner:** `search-eval`
- **Earliest wave:** W4
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `level` | `enum` | `info` | `file, env, CLI` | `APPLY_LIVE` |
| `rotation_bytes` | `u64` | `10485760; bounded` | `file` | `APPLY_LIVE` |
| `rotation_files` | `u16` | `5; bounded` | `file` | `APPLY_LIVE` |
| `include_query_text` | `bool` | `false; fixed` | `none` | `REJECT` |
| `include_source_content` | `bool` | `false; fixed` | `none` | `REJECT` |
| `include_absolute_paths` | `bool` | `false; fixed default` | `none` | `REJECT` |
| `privileged_debug_enabled` | `bool` | `false` | `explicit CLI + binding auth` | `SECURITY_BARRIER` |
| `privileged_debug_ttl_ms` | `u64` | `300000; finite` | `explicit CLI` | `APPLY_LIVE restrictive` |

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

- default telemetry is content-minimized
- privileged debug is binding-scoped and finite
- secrets/source/query/raw paths never enter ordinary logs

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
