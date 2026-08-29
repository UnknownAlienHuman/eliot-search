# `[secrets]` configuration contract

- **Owner:** `search-os-secrets`
- **Earliest wave:** W1
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `opaque_refs_only`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `backend` | `enum` | `windows_user_bound` | `file` | `RESTART_DEPENDENCY` |
| `lease_ttl_ms` | `u64` | `30000; 1000..300000` | `file` | `APPLY_LIVE decrease / RESTART increase` |
| `rotation_grace_ms` | `u64` | `60000; 0..600000` | `file` | `APPLY_LIVE` |
| `qdrant_api_secret_ref` | `SecretRef | absent` | `absent in DIRECT` | `file, whitelisted env` | `RESTART_DEPENDENCY` |

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

- plaintext, DPAPI blobs and command-line secrets are rejected
- references are bound to user, installation, incarnation and purpose
- redacted output never resolves a SecretRef

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
