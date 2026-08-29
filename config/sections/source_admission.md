# `[source_admission]` configuration contract

- **Owner:** `search-source-admission`
- **Earliest wave:** W2
- **Section minimum:** `SECURITY_BARRIER`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `bootstrap_policy_profile` | `ProfileId` | `baseline-safe-v1` | `file` | `SECURITY_BARRIER` |
| `max_file_bytes` | `u64` | `16777216; downward live` | `file` | `SECURITY_BARRIER` |
| `allow_generated` | `bool` | `false` | `file` | `SECURITY_BARRIER + reconcile` |
| `allow_vendor` | `bool` | `false` | `file` | `SECURITY_BARRIER + reconcile` |
| `allow_binary` | `bool` | `false` | `file` | `SECURITY_BARRIER + reconcile` |

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

- known credential/private-key classes remain deny-by-default
- change creates a policy revision, not implicit membership mutation
- permissive changes require explicit reconcile/admission

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
