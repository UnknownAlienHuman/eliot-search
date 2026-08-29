# `[instance]` configuration contract

- **Owner:** `search-runtime-owner`
- **Earliest wave:** W1
- **Section minimum:** `DRAIN_AND_RESTART`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `mode` | `standalone | managed_client` | `standalone` | `file, CLI` | `DRAIN_AND_RESTART` |
| `data_root` | `local path | absent` | `OS per-user local default` | `file, env, CLI` | `DRAIN_AND_RESTART` |
| `lock_timeout_ms` | `u64` | `0; max 30000` | `file, env, CLI` | `RESTART_DEPENDENCY` |
| `shutdown_timeout_ms` | `u64` | `15000; 1000..120000` | `file, env, CLI` | `APPLY_LIVE` |
| `abandoned_owner_policy` | `enum` | `quarantine_unless_identity_absent` | `file` | `RESTART_DEPENDENCY` |

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

- one data-root owner; mode changes never apply live
- network/remote data roots are rejected in baseline
- ordinary diagnostics expose only path digest/location class

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
