# Function contract — `search-config`

**Status:** W1 pure implementation complete; concrete acquisition and runtime application remain caller-owned.

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

## Implemented representation notes

- Optional capability fields use an explicit `ConfigValue::Absent`; absence is never inferred from a missing higher-precedence key.
- `ConfigFingerprint` remains SHA-256 over the exact legacy v1 length-delimited preimage containing schema/profile identities, descriptor revisions/digests and package-validated section digests. Shared `sha256_exact_bytes` executes the accepted SHA-2 donor without adding framing. The preimage has a hard 8 MiB ceiling in addition to the caller ceiling. This is not the forthcoming canonical v2 identity under #238.
- The parser accepts the finite baseline TOML subset used by the project configuration: top-level `schema_version` and `profile`, flat package tables, booleans, signed integers, strings and arrays of strings. Unsupported TOML constructs fail closed.
- Ordinary `Debug` representations redact unvalidated text, paths and opaque secret references.
- File parsing and `ConfigDocument::from_entries` share one private construction budget. Sections (including empty file tables), entries and fields per section are bounded before map growth or section clones. Parsed strings use the same legacy decoding rules for an allocation-free UTF-8 length pass and the subsequent exact-capacity allocation; list item counts are charged before each item decode/push.
- `ConfigLimits::W1` adds aggregate ceilings: identifier bytes 1 MiB, decoded value bytes 4 MiB and list items 8,192 per layer. Identifiers count the profile once, declared section names once and both path components for each entry. Value bytes count text/list UTF-8 plus boolean=1 and integer=8; resets contribute zero. Per-identifier/text/list limits also apply to caller-owned captured values before configuration collection growth. These are logical payload/count ceilings, not an allocator-resident-byte measurement.
- Constructor header rules remain unchanged: positive schema, no Defaults document, profiles only on File. File profile is optional in `from_entries` and mandatory in `parse_document`. Bounds now reject inputs that previously bypassed sections/fields/per-value/aggregate checks. In-budget v1 values and fingerprint bytes remain unchanged; over-limit malformed/duplicate intersections may report capacity first. Fixed content-free errors require no diagnostic path buffer. TOML donor/v2, diagnostic byte ceilings, section-digest authority and ordered fingerprint ownership remain #238/#389 follow-ups.

## Operations

### `parse_document(bytes, source, limits) -> Result<ConfigDocument, ConfigError>`

Validates framing, encoding, schema version, duplicate keys/tables, nesting and allocation limits.
It performs no path/environment expansion and never reads another source.

### `ConfigDocument::from_entries(schema_version, requested_profile, source, entries, limits)`

Admits caller-owned captured typed entries through the same private budget as file parsing, without cloning text/list values. Checks duplicate paths before insertion; failures return no partial document. Acquisition and source-digest encoding remain caller-owned. Explicit `ConfigLimits` struct literals must supply the three new aggregate fields.

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
location classes. A path exceeding the shared 8 MiB digest ceiling returns only text-kind/byte-extent
metadata, with no digest or raw path. Ordinary bounded paths retain their exact standard SHA-256 bytes.

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
