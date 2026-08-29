# `search-config` implementation packet

**Path:** `crates/search-config`  
**Capability:** W1 configuration support  
**Delivery:** W1 / P01  
**Gate:** BLOCKED until W0 contracts handoff is accepted  
**Direct public handoffs:** `search-contracts`

Apply `../ASSIGNMENT_PROTOCOL.md`. Read `docs/config/CONFIGURATION_1.0.md` and
`config/sections.toml`. Logical names express required semantics, not mandatory Rust spelling.

## Mission

Provide deterministic configuration layering, section dispatch, security-floor enforcement,
redaction and reconfiguration planning while each capability retains ownership of its settings and
runtime state.

## Owns

- `ConfigSource`, `ConfigLayer`, `ConfigDocument`, `ConfigSectionName`, `ConfigKeyPath`
- deterministic defaults < file < environment < CLI precedence
- override allowlists and immutable/fixed setting enforcement
- section descriptor registry and duplicate/unknown rejection
- effective snapshot canonicalization and `ConfigFingerprint`
- config diff and ordered reload plan
- redacted effective-config diagnostics

## Must not own

- reading files, environment variables or process arguments
- package-specific settings structs, default policies or runtime state
- secret plaintext, secret resolution or credential transport
- optional-provider authorization
- applying reload plans or committing security mutations

## Logical operations

1. `parse_document(bytes, source) -> Result<ConfigDocument, ConfigError>`
2. `register_sections(descriptors) -> Result<ConfigRegistry, ConfigError>`
3. `merge_layers(defaults, file, environment, cli, registry) -> Result<MergedConfig, ConfigError>`
4. `project_section(merged, descriptor) -> Result<ConfigSectionInput, ConfigError>`
5. `assemble_effective(validated_sections) -> Result<EffectiveConfigSnapshot, ConfigError>`
6. `fingerprint(snapshot) -> ConfigFingerprint`
7. `diff(old, new) -> ConfigDelta`
8. `plan_reconfiguration(delta, descriptors) -> Result<ReconfigurationPlan, ConfigError>`
9. `redacted_view(snapshot, disclosure) -> RedactedConfigView`
10. `validate_environment_key(name, registry) -> Result<ConfigKeyPath, ConfigError>`

## Operation semantics

- Parsing is bounded before allocation, rejects duplicate keys, non-UTF-8, unsupported schema version
  and unknown load-bearing top-level fields.
- Merge order is deterministic. A higher layer may override only a key whose descriptor explicitly
  permits that source; arrays/maps are file-only unless a descriptor says otherwise.
- Package section validation happens before an effective snapshot exists. One invalid section rejects
  the whole candidate snapshot.
- Fingerprints use canonical bytes, include schema/section descriptor revisions and exclude secret
  plaintext because plaintext is forbidden.
- Reconfiguration plans are ordered and monotonic: `REJECT` > `SECURITY_BARRIER` >
  `NEW_COLLECTION_GENERATION` > `DRAIN_AND_RESTART` > `RESTART_DEPENDENCY` > `APPLY_LIVE` > `NOOP`.
- Planning does not execute changes. The daemon/capability owner performs the plan and produces receipts.

## Required invariants

- unknown config section/key fails closed
- fixed security floors cannot be weakened by file, environment or CLI
- only opaque `SecretRef` values are accepted in secret-bearing fields
- config diagnostics never contain source content, query text, absolute paths by default or secret values
- equal layers/descriptors produce byte-identical fingerprints and plans
- optional model/document settings are rejected without accepted gate/ADR/artifact prerequisites
- a change requiring restart, security barrier, collection generation or rebuild is never applied live

## Typed failure surface

- `CONFIG_PARSE_FAILED`
- `CONFIG_SCHEMA_VERSION_UNSUPPORTED`
- `CONFIG_DUPLICATE_KEY`
- `CONFIG_UNKNOWN_SECTION`
- `CONFIG_UNKNOWN_KEY`
- `CONFIG_OVERRIDE_NOT_ALLOWED`
- `CONFIG_SECURITY_FLOOR_VIOLATION`
- `CONFIG_SECRET_PLAINTEXT_FORBIDDEN`
- `CONFIG_SECTION_CONFLICT`
- `CONFIG_SECTION_INVALID`
- `CONFIG_PROFILE_NOT_AUTHORIZED`
- `CONFIG_RECONFIGURATION_REJECTED`

## Exit tests / evidence

- `layer_precedence_and_override_allowlist`
- `duplicate_unknown_and_wrong_type_fail_closed`
- `plaintext_secret_rejected_and_redacted_view_safe`
- `fixed_security_floor_cannot_be_weakened`
- `equal_inputs_equal_fingerprint_and_reload_plan`
- `restart_rebuild_generation_and_security_changes_never_live`
- `optional_profile_requires_gate_adr_and_artifact_receipts`
- `package_section_collision_rejected`
- `parser_and_merge_are_io_free`

## Suggested internal modules

```text
search-config/src/
  source.rs
  document.rs
  registry.rs
  merge.rs
  section.rs
  snapshot.rs
  fingerprint.rs
  diff.rs
  reload.rs
  redact.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤5,500 hand-written lines**.
- Split review: **before 8,500 total hand-written lines**.
- Hard stop: **10,000 including package-local tests**.
- Parsing/layering/reload planning remain together while one effective-config identity governs them.
  A format-specific parser may split only if it becomes independently replaceable.
