# `[qdrant_process]` configuration contract

- **Owner:** `search-qdrant-supervisor`
- **Earliest wave:** W3
- **Section minimum:** `RESTART_DEPENDENCY`
- **Secret policy:** `opaque_refs_only`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `enabled` | `bool` | `false in DIRECT` | `file, CLI` | `RESTART_DEPENDENCY` |
| `artifact_path` | `local absolute path` | `required when enabled` | `file` | `RESTART_DEPENDENCY` |
| `expected_sha256` | `Sha256Digest32` | `required when enabled` | `file` | `RESTART_DEPENDENCY` |
| `expected_version` | `BoundedText` | `required when enabled` | `file` | `RESTART_DEPENDENCY` |
| `api_secret_ref` | `SecretRef` | `required when enabled` | `file, whitelisted env` | `RESTART_DEPENDENCY` |
| `port` | `u16` | `0 supervised selection; loopback only` | `file` | `RESTART_DEPENDENCY` |
| `startup_timeout_ms` | `u64` | `30000; bounded` | `file` | `APPLY_LIVE for next start` |
| `health_timeout_ms` | `u64` | `2000; bounded` | `file` | `APPLY_LIVE` |
| `restart_max_attempts` | `u8` | `3; 0..10` | `file` | `APPLY_LIVE` |
| `restart_window_ms` | `u64` | `300000; bounded` | `file` | `APPLY_LIVE` |
| `auto_download` | `bool` | `false; fixed` | `none` | `REJECT` |
| `auto_upgrade` | `bool` | `false; fixed` | `none` | `REJECT` |

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

- enabled requires exact path/version/SHA-256/SecretRef and accepted qualification
- bind is loopback-only and process identity is verified
- automatic download/upgrade and plaintext credentials are impossible

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
