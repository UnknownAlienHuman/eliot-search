# Function contract — `search-config`

**Status:** W1 pure implementation complete; concrete acquisition and runtime application remain caller-owned.

All functions are pure and I/O-free. Callers capture file bytes, environment variables, CLI values,
platform facts, accepted capability receipts and time before invoking this crate.

## Shared rules

- Input bytes are bounded before allocation and must be UTF-8 without BOM under [TOML_PROFILE_2.md](TOML_PROFILE_2.md).
- Precedence is `defaults < file < environment < cli`, subject to each field's override allowlist.
- One invalid section rejects the whole candidate snapshot.
- Unknown sections, keys, duplicate keys and unknown load-bearing fields fail closed.
- Plaintext secrets are invalid in every layer. Only explicitly typed opaque `SecretRef` values may pass.
- Every returned diagnostic is redacted and bounded.
- Equal canonical inputs produce byte-identical snapshots, fingerprints, deltas and plans.

## Implemented representation notes

- Optional capability fields use an explicit `ConfigValue::Absent`; absence is never inferred from a missing higher-precedence key.
- New effective snapshots compute config-owned BLAKE3/shared deterministic CBOR v2. ConfigFingerprint preserves algorithm/profile/revision in equality and serialization. restore_legacy_v1 is decode-only and cannot create a current snapshot; source must be rebuilt. FILE schema2 and fingerprint revision2 are independent version dimensions (VERSIONED_REBUILD); schema2 changes the snapshot header value and digest, preserving its row layout/domain/tag.
- File parsing delegates syntax to the maintained `toml_parser` event parser. Root `schema_version=2` and `profile` appear once before flat bare-name tables. Bare keys, booleans, decimal i64, single-line strings and flat string arrays are supported; quoted/dotted keys, nested/array/inline tables, nested arrays, multiline strings, floats/datetimes and non-decimal integers fail closed. Schema1 files return `UnsupportedSchemaVersion`.
- Ordinary `Debug` representations redact unvalidated text, paths and opaque secret references.
- `ConfigDocument` Debug emits only schema, profile presence/byte extent, source kind and entry/declared-section counts. It does not traverse captured profile text, names, values, source refs or digests. The same representation applies when `ConfigLayers` embeds the document; its separate externally selected `requested_profile` remains caller-authorized metadata. Debug changes no data/accessor or persisted v1 bytes and constructs no internal diagnostic String.
- File parsing and `ConfigDocument::from_entries` share one private construction budget. Sections (including empty file tables), entries and fields per section are bounded before map growth or section clones. Strings use the actual donor decoder for an allocation-free UTF-8 count, builder charge and exact-capacity fill/length check; list item counts are charged before each item decode/push. Decimal normalization uses a fixed stack sink and native i64 conversion. No copied string/numeric grammar remains.
- `ConfigLimits::W1` adds aggregate ceilings: identifier bytes 1 MiB, decoded value bytes 4 MiB and list items 8,192 per layer. Identifiers count the profile once, declared section names once and both path components for each entry. Value bytes count text/list UTF-8 plus boolean=1 and integer=8; resets contribute zero. Per-identifier/text/list limits also apply to caller-owned captured values before configuration collection growth. These are logical payload/count ceilings, not an allocator-resident-byte measurement.
- Typed constructor header rules remain unchanged: positive schema, no Defaults document, profiles only on File. File profile is optional in `from_entries` and mandatory in `parse_document`; typed captures make no TOML syntax claim. File grammar is `eliot-config-toml@2` only, with both root headers required before the first table. W1 adds 131,072 lexer tokens and 4 MiB logical token-storage ceilings, checked before reservation/push. Fixed content-free errors require no diagnostic path buffer; sticky failure prevents subsequent growth/success even if the donor continues. Canonical rows, tags and layer/descriptor/secret semantics remain unchanged; schema2 deliberately changes the snapshot header value. Fixture execution remains deferred.

## Operations

### `parse_document(bytes, source, limits) -> Result<ConfigDocument, ConfigError>`

Validates byte/UTF-8/BOM/LF/CRLF framing, bounded donor tokens and the sole schema2 file profile,
duplicate keys/tables, value shape/depth and decoded collection budgets. Lone CR is rejected.
It performs no path/environment expansion and never reads another source. Failure returns no
partial document or donor diagnostic. Full grammar and migration rules are in TOML_PROFILE_2.md.

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

### Private canonical section and snapshot compute

ValidatedSection::new(input, selected_profile, limits) computes a fallible section equality digest internally. The public caller-digest parameter, arbitrary fingerprint iterator and package canonical writer are removed. Private snapshot compute hashes the existing finite ordered BTreeMap through shared blake3_canonical_array; plaintext secrets remain structurally forbidden.

Section domain is `eliot/cbor/config-section/v2`. Its ordered root contains one header row
`[name, owner, descriptor_revision, registry_digest, requested_profile, selected_profile]`,
then field rows `[key, value_kind, payload, source_kind, source_ref, source_digest, reset_bool]`.
Value kinds are Absent0/Boolean1/Integer2/Text3/SecretReference4/StringList5; source kinds use
Defaults0/File1/Environment2/Cli3. Negative integers use I64, nonnegative U64; absent is Null;
opaque references are text and list order is retained. False reset is explicit.

Snapshot domain/profile is `eliot/cbor/config-snapshot/v2`. Its header is
`[config_schema_version, selected_profile]`, followed by ordered section rows
`[name, owner, descriptor_revision, registry_digest, section_v2_digest]`.
Both root counts include the header: at most4,096 rows, hence4,095 fields/sections.

Borrowed identifiers/profile/source refs, value payload and list counts are checked against
ConfigLimits before row clones/list growth; assembly also charges before name/owner clones
and map insertion. Logical construction sums fixed revision8, digest32, kind8, source-kind8,
reset1 plus borrowed payload under the caller/shared8MiB ceiling. This is not a CBOR estimator
or heap measurement. The shared encoder alone checks exact domain-inclusive bytes/depth.
Failures return content-free errors and no partial identity. These are equality identities
over exact supplied provenance; they do not prove package validation or runtime effects.

### `diff(old, new) -> Result<ConfigDelta, ConfigError>`

Produces stable key-path deltas and marks restrictive changes. It never treats absence as reset unless
the field schema explicitly defines reset semantics.

### `plan_reconfiguration(delta, descriptors) -> Result<ReconfigurationPlan, ConfigError>`

Preserves every required obligation. A plan may combine security barriers, dependency restarts,
projection rebuild and collection-generation cutover; it must not collapse them to a lossy scalar.

### `redacted_view(snapshot, disclosure) -> Result<RedactedConfigView, ConfigError>`

Returns schema/fingerprint/owner/action metadata and allowed scalar summaries only. Secret refs, raw
environment values, source/query content and absolute paths are excluded or represented by digests and
location classes. A path exceeding the shared 8 MiB digest ceiling returns only text-kind/byte-extent
metadata, with no digest or raw path. Ordinary bounded paths retain their exact standard SHA-256 bytes.

Both entry count and complete logical payload are charged before profile/path/text clones, path hashing
or map growth. W1 `max_diagnostic_bytes` is 1 MiB. The header charges fingerprint=32 + algorithm/profile UTF-8 + revision4 bytes, selected
profile UTF-8 bytes and omitted count=8. Each entry charges section/key UTF-8 bytes plus value tag=1
and payload: absent/hidden=0, boolean=1, integer=8, visible text bytes plus truncation flag=1, list
extents=16, path class/digest=33, metadata kind/extent=9. These architecture-independent logical bytes
exclude serialization framing/escaping and allocator/map overhead; this is not a measured heap bound.
Overflow or insufficient header capacity returns an error without a partial view; invalid limits
reject. An entry exceeding either ceiling is omitted whole; later smaller entries may still fit in
canonical order. The daemon status adapter maps a failed view to a fixed content-free read-only error
record. Runtime readiness calculations remain unchanged. The immediate status schema is eliot.effective-config-status-v2 and preserves fingerprint algorithm/profile/revision, rejecting a report/snapshot identity mismatch. It reports the effective snapshot schema and supported_file_syntax_profile metadata; that support value does not assert captured-file provenance.

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
