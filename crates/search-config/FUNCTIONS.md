# Function contract — `search-config`

**Status:** logical implementation contract; no runtime behavior is implemented yet.

All functions are pure and I/O-free. Callers capture file bytes, environment variables, CLI values,
platform facts, accepted capability receipts and time before invoking this crate.

## Shared rules

- Input bytes are bounded before allocation and must be UTF-8 TOML 1.0 without BOM.
- Precedence is `defaults < file < environment < cli`, subject to each field's override allowlist.
- One invalid section rejects the whole candidate snapshot.
- Unknown sections, keys, duplicate keys and unknown load-bearing fields fail closed.
- Plaintext secrets are invalid in every layer. Only explicitly typed opaque `SecretRef` values may pass.
- Every returned diagnostic is redacted and bounded.
- Equal canonical inputs produce byte-identical snapshots, fingerprints, deltas and plans.

## Operations

### `parse_document(bytes, source, limits) -> Result<ConfigDocument, ConfigError>`

Validates framing, encoding, schema version, duplicate keys/tables, nesting and allocation limits.
It performs no path/environment expansion and never reads another source.

### `register_sections(descriptors) -> Result<ConfigRegistry, ConfigError>`

Rejects duplicate names, duplicate field paths, unknown reload classes, owner mismatches and conflicting
secret policies. Registration order cannot affect the registry digest.

### `merge_layers(defaults, file, environment, cli, registry) -> Result<MergedConfig, ConfigError>`

Applies deterministic field-level precedence. Arrays, maps, policies, artifacts and secrets remain
file-only unless the descriptor explicitly permits a bounded alternate representation.

### `project_section(merged, descriptor) -> Result<ConfigSectionInput, ConfigError>`

Returns only keys owned by the target descriptor, with field provenance and explicit reset markers.
A capability cannot inspect another package's section through this operation.

### `assemble_effective(validated_sections, selected_profile) -> Result<EffectiveConfigSnapshot, ConfigError>`

Requires one validated result for every registered effective section. Rejects missing, duplicate,
stale-descriptor and profile/gate mismatches.

### `fingerprint(snapshot) -> ConfigFingerprint`

Hashes canonical schema/profile/descriptor revisions and validated section digests. It never hashes
secret plaintext because plaintext is structurally forbidden.

### `diff(old, new) -> Result<ConfigDelta, ConfigError>`

Produces stable key-path deltas and marks restrictive changes. It never treats absence as reset unless
the field schema explicitly defines reset semantics.

### `plan_reconfiguration(delta, descriptors) -> Result<ReconfigurationPlan, ConfigError>`

Preserves every required obligation. A plan may combine security barriers, dependency restarts,
projection rebuild and collection-generation cutover; it must not collapse them to a lossy scalar.

### `redacted_view(snapshot, disclosure) -> RedactedConfigView`

Returns schema/fingerprint/owner/action metadata and allowed scalar summaries only. Secret refs, raw
environment values, source/query content and absolute paths are excluded or represented by digests and
location classes.

### `validate_environment_key(name, registry) -> Result<ConfigKeyPath, ConfigError>`

Accepts only exact `ELIOT_SEARCH__SECTION__KEY` mappings for registered scalar fields. Unknown prefixed
variables are startup errors rather than ignored typos.

## Failure and retry semantics

Pure failures are deterministic and never partially publish state. Retrying identical inputs returns
the same result. Cancellation and deadlines are caller concerns because these operations are bounded
CPU-only work; implementations still accept an optional budget/cancellation checkpoint for hostile
input protection.

## Required conformance fixtures

`same_layers_same_fingerprint`, `duplicate_unknown_wrong_type_fail_closed`,
`override_allowlist_enforced`, `plaintext_secret_rejected_every_layer`,
`package_section_collision_rejected`, `mixed_reconfiguration_obligations_preserved`,
`redacted_view_non_disclosure`, and `parser_merge_are_io_free`.
