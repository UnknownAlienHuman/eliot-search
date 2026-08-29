# `[reconcile]` configuration contract

- **Owner:** `search-source-reconcile`
- **Earliest wave:** W5
- **Section minimum:** `APPLY_LIVE`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `inventory_interval_ms` | `u64` | `300000; 10000..86400000` | `file` | `APPLY_LIVE` |
| `overflow_reconcile_budget_ms` | `u64` | `5000; bounded` | `file` | `APPLY_LIVE` |
| `max_roots_per_sweep` | `u32` | `64; 1..4096` | `file` | `APPLY_LIVE` |
| `max_entries_per_slice` | `u32` | `10000; bounded` | `file` | `APPLY_LIVE` |

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

- no interval/budget turns an observation gap into currentness
- limits are finite and slices remain cancellable
- overflow always schedules authoritative reconciliation

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
