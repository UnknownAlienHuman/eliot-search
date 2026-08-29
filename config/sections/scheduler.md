# `[scheduler]` configuration contract

- **Owner:** `search-retrieval-executor`
- **Earliest wave:** W4
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `interactive_concurrency` | `u16` | `8; host-bounded` | `file` | `APPLY_LIVE` |
| `verification_concurrency` | `u16` | `2; host-bounded` | `file` | `APPLY_LIVE` |
| `background_concurrency` | `u16` | `2; host-bounded` | `file` | `APPLY_LIVE` |
| `interactive_queue` | `u32` | `64; finite` | `file` | `APPLY_LIVE` |
| `verification_queue` | `u32` | `16; finite` | `file` | `APPLY_LIVE` |
| `background_queue` | `u32` | `32; finite` | `file` | `APPLY_LIVE` |
| `background_pause_cpu_percent` | `u8` | `80; 1..100` | `file` | `APPLY_LIVE` |
| `background_pause_memory_percent` | `u8` | `85; 1..100` | `file` | `APPLY_LIVE` |

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

- all queues and concurrency limits are finite
- interactive work retains priority
- saturation produces typed rejection/partial coverage

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
