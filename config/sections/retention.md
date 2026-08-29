# `[retention]` configuration contract

- **Owner:** `search-retention`
- **Earliest wave:** W7
- **Section minimum:** `SECURITY_BARRIER`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `sweep_interval_ms` | `u64` | `3600000; bounded` | `file` | `APPLY_LIVE` |
| `mark_batch_objects` | `u32` | `10000; finite` | `file` | `APPLY_LIVE` |
| `delete_batch_objects` | `u32` | `1000; finite` | `file` | `APPLY_LIVE` |
| `default_retention_ms` | `u64` | `finite policy value` | `file` | `SECURITY_BARRIER` |
| `claim_secure_erase` | `bool` | `false; fixed` | `none` | `REJECT` |

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

- legal holds/purge/tombstones are durable commands, not config toggles
- ordinary reclaim is not purge
- secure erase is never claimed without evidence

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
