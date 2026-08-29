# `[protocol]` configuration contract

- **Owner:** `search-provider-protocol`
- **Earliest wave:** W1
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `max_frame_bytes` | `u32` | `8388608; downward only` | `file, env, CLI` | `RESTART_DEPENDENCY` |
| `max_in_flight_per_connection` | `u16` | `32; 1..32` | `file, env, CLI` | `APPLY_LIVE decrease / RESTART increase` |
| `hello_timeout_ms` | `u64` | `5000; 100..30000` | `file` | `APPLY_LIVE` |
| `request_deadline_cap_ms` | `u64` | `120000; 100..600000` | `file` | `APPLY_LIVE` |
| `compression` | `bool` | `false; fixed` | `none` | `REJECT` |
| `fragmented_messages` | `bool` | `false; fixed` | `none` | `REJECT` |
| `pairing_required` | `bool` | `true; fixed` | `none` | `REJECT` |

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

- 8 MiB and 32 are architecture ceilings, not suggestions
- no second unauthenticated endpoint
- pairing/authentication cannot be disabled by configuration

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
