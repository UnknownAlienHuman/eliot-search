# `[control]` configuration contract

- **Owner:** `search-control-redb`
- **Earliest wave:** W1
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `open_timeout_ms` | `u64` | `5000; 100..60000` | `file` | `RESTART_DEPENDENCY` |
| `max_idempotency_records` | `u32` | `10000; 100..1000000` | `file` | `APPLY_LIVE` |
| `idempotency_ttl_ms` | `u64` | `86400000; operation-class bounded` | `file` | `APPLY_LIVE` |
| `snapshot_publish_timeout_ms` | `u64` | `2000; 100..30000` | `file` | `APPLY_LIVE` |
| `durability` | `enum` | `fsync_atomic; fixed` | `none` | `REJECT` |
| `migration_policy` | `enum` | `verified_or_quarantine; fixed` | `none` | `REJECT` |

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

- journal stays under owned data root
- hot reads never create durable rows
- durability and quarantine floors cannot be weakened

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
