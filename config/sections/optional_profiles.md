# `[optional_profiles]` configuration contract

- **Owner:** `eliot-searchd`
- **Earliest wave:** W10
- **Section minimum:** `GATE_REQUIRED`
- **Secret policy:** `opaque_refs_only`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `semantic` | `bool` | `false` | `file` | `GATE_REQUIRED` |
| `document` | `bool` | `false` | `file` | `GATE_REQUIRED` |
| `model_provider_profile` | `ProfileId | null` | `null` | `file` | `GATE_REQUIRED` |
| `document_provider_profile` | `ProfileId | null` | `null` | `file` | `GATE_REQUIRED` |

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

- requires accepted G5/P15, dedicated ADR, exact qualification and Cargo feature
- configuration never authorizes optional depth
- disabled baseline remains fully functional

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
