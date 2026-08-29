# `[revision_store]` configuration contract

- **Owner:** `search-revision-store`
- **Earliest wave:** W2
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `max_total_bytes` | `u64` | `21474836480; local quota` | `file` | `APPLY_LIVE decrease after safe plan` |
| `max_single_object_bytes` | `u64` | `16777216; covers admitted ceiling` | `file` | `RESTART_DEPENDENCY` |
| `fsync_before_publish` | `bool` | `true; fixed` | `none` | `REJECT` |
| `verify_after_reopen` | `bool` | `true; fixed` | `none` | `REJECT` |
| `temporary_subdir` | `relative component` | `tmp under data root` | `file` | `RESTART_DEPENDENCY` |

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

- lowering quota never deletes reachable objects immediately
- temporary storage cannot escape data root
- fsync/reopen verification floors are fixed

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
