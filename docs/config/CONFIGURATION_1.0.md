# ELIOT Search local configuration 1.0

**Status:** implementation contract; runtime parsing and reload are not implemented.  
**Owner of layering:** `search-config`.  
**Owner of each typed section:** the package listed in `config/sections.toml`.  
**Composition owner:** `eliot-searchd`.

## 1. Boundary

Configuration selects and bounds already-authorized Search capabilities. It does not create source
membership, access authority, publication truth, purge authority or optional-provider acceptance.

Three distinct responsibilities are enforced:

```text
input acquisition                         daemon / CLI
  file bytes, selected environment, CLI overrides

layering and reconfiguration planning     search-config
  parse, merge, registry, fingerprint, redaction, diff, action plan

section meaning and runtime application   owning capability package
  typed settings, defaults, field validation, apply/rollback receipt
```

`eliot-searchd` may wire these responsibilities but must not implement section policy in composition
code.

## 2. Document format and limits

The baseline local file is UTF-8 TOML 1.0 with:

- no byte-order mark;
- duplicate keys/tables rejected;
- schema version required before package section decoding;
- unknown load-bearing top-level sections and fields rejected;
- finite nesting and collection bounds enforced before full allocation;
- no implicit path, environment-variable or home-directory expansion inside `search-config`;
- no plaintext secret, source body, query text or credential value.

The package-specific parser dependency is chosen and pinned during implementation. The contract does
not authorize a floating parser version.

```yaml
ConfigDocument:
  schema_version: 1
  profile: direct | lexical | code | semantic_optional | document_optional
  sections: bounded_map<ConfigSectionName, ConfigSectionInput>
```

A profile name is a requested composition ceiling. The effective profile is the intersection of the
request, accepted gates, compiled Cargo features, qualified artifacts and current capability health.
Configuration cannot widen it.

## 3. Layers and precedence

```text
compiled security-safe defaults
  < local file
  < whitelisted environment overrides
  < explicit CLI overrides
```

A later layer wins only when the field descriptor authorizes that source. Precedence never overrides:

- fixed architecture/security values;
- an accepted artifact digest or capability receipt;
- a package-owned minimum/maximum bound;
- a security floor;
- file-only arrays/maps;
- `SecretRef` requirements;
- optional-profile gates.

### 3.1 Environment mapping

The only recognized prefix is:

```text
ELIOT_SEARCH__SECTION__KEY
```

Names are ASCII upper-case and map to registered scalar keys only. Unknown prefixed variables fail
startup rather than being ignored as typos. Lists, maps, nested policy objects, artifact manifests and
secret values are file-only unless their section explicitly defines another bounded representation.

Examples:

```text
ELIOT_SEARCH__INSTANCE__MODE=standalone
ELIOT_SEARCH__PROTOCOL__MAX_IN_FLIGHT_PER_CONNECTION=16
ELIOT_SEARCH__OBSERVABILITY__LEVEL=debug
```

An environment value may contain an opaque `SecretRef` identifier only where the field explicitly
allows it. API keys, tokens and private-key material are always rejected.

### 3.2 CLI overrides

CLI overrides are closed typed flags, not arbitrary `--set key=value`. Unknown keys, raw TOML fragments
and generic JSON maps are forbidden. The CLI produces a `ConfigLayer`; it does not mutate runtime state.

## 4. Generic configuration records

```yaml
ConfigSource:
  kind: defaults | file | environment | cli
  source_ref: opaque
  source_digest: Blake3Digest32

ConfigSectionDescriptor:
  section_name: ConfigSectionName
  owner_package: PackageName
  schema_revision: NonZeroRevision
  minimum_reload_class: ReloadClass
  allowed_override_sources: bounded_set<ConfigSourceKind>
  field_registry_digest: Blake3Digest32
  secret_policy: forbid_plaintext | opaque_refs_only

EffectiveConfigSnapshot:
  config_schema_version: u32
  selected_profile: SearchProfile
  section_descriptors: bounded_list<ConfigSectionDescriptor>
  validated_section_digests: bounded_map<ConfigSectionName, Blake3Digest32>
  config_fingerprint: ConfigFingerprint

ConfigDelta:
  old_fingerprint: ConfigFingerprint
  new_fingerprint: ConfigFingerprint
  changed_sections: bounded_list<SectionDelta>

SectionDelta:
  section_name: ConfigSectionName
  changed_key_paths: bounded_set<ConfigKeyPath>
  minimum_action: ReloadClass
  restrictive: bool

ReconfigurationPlan:
  old_fingerprint: ConfigFingerprint
  new_fingerprint: ConfigFingerprint
  dominant_action: ReloadClass
  ordered_steps: bounded_list<ReconfigurationStep>
  affected_capabilities: bounded_set<PackageName>
  required_receipts: bounded_set<ReceiptKind>
```

## 5. Reload classes

Ordered from least to most restrictive:

```text
NOOP
APPLY_LIVE
SECURITY_BARRIER
RESTART_DEPENDENCY
DRAIN_AND_RESTART
NEW_COLLECTION_GENERATION
REBUILD_PROJECTION
GATE_REQUIRED
REJECT
```

The most restrictive changed field dominates. A package may raise, never lower, the section minimum.

### `APPLY_LIVE`

The owner validates a candidate settings object, atomically swaps an immutable settings snapshot and
returns an application receipt. In-flight work retains the old snapshot or revalidates explicitly; it
never observes a partially mutated object.

### `SECURITY_BARRIER`

Used for restrictive admission, disclosure, retention or related settings. The owner performs durable
commit, live snapshot publication and dependent invalidation before acknowledgement. Permissive changes
do not retroactively admit data; they create a new policy revision and explicit reconcile/rebuild work.

### `RESTART_DEPENDENCY`

The daemon drains the affected dependency, starts it with the new settings, reruns qualification and
publishes readiness. Failure retains direct/degraded operation or quarantines; it does not silently use
old credentials/artifacts under the new fingerprint.

### `DRAIN_AND_RESTART`

Used for data-root, owner mode, local transport identity or changes affecting process-wide ownership.
Request admission stops, work drains, handles/pins/processes close in order and a new owner incarnation
starts.

### `NEW_COLLECTION_GENERATION`

Used for lexical provider, payload schema, point identity, Qdrant build or projection compatibility
changes. A new collection generation is built and guarded route cutover occurs; in-place reinterpretation
of old points is forbidden.

### `REBUILD_PROJECTION`

Used when source truth remains compatible but derived projection output changes. Publication creates a
new epoch or generation as required by point/profile identity.

### `GATE_REQUIRED`

Optional semantic/document configuration remains rejected until the accepted Product Pulse, dedicated
ADR, exact artifact qualification and enabled Cargo feature are all present.

### `REJECT`

Used for unknown fields, plaintext secrets, security-floor weakening, invalid bounds, missing artifact
identity, unsupported platform behavior or an unrepresentable mixed change.

## 6. Section contracts

The values below are initial implementation defaults and bounds. A package handoff may narrow them but
cannot widen fixed architecture/security rules without an ADR or architecture revision.

### 6.1 `[instance]` — `search-runtime-owner`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `mode` | `standalone | managed_client` | `standalone` | file, CLI | drain/restart |
| `data_root` | local path or absent | OS per-user local default; no network/remote URI baseline | file, env, CLI | drain/restart |
| `lock_timeout_ms` | `u64` | `0`; maximum `30000` | file, env, CLI | restart |
| `shutdown_timeout_ms` | `u64` | `15000`; `1000..120000` | file, env, CLI | live for next shutdown |
| `abandoned_owner_policy` | enum | `quarantine_unless_identity_absent` | file only | restart |

Final path resolution, reparse handling and process identity are runtime-owner responsibilities. The
redacted config exposes only a path digest and location class by default.

### 6.2 `[secrets]` — `search-os-secrets`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `backend` | enum | `windows_user_bound`; fixed on Windows baseline | file only | restart |
| `lease_ttl_ms` | `u64` | `30000`; `1000..300000` | file | live decrease; restart increase |
| `rotation_grace_ms` | `u64` | `60000`; `0..600000` | file | live |
| `qdrant_api_secret_ref` | `SecretRef` or absent | absent in DIRECT | file, whitelisted env | restart dependency |

Plaintext values, DPAPI blobs, environment-expanded credentials and command-line secrets are invalid.

### 6.3 `[control]` — `search-control-redb`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `open_timeout_ms` | `u64` | `5000`; `100..60000` | file | restart |
| `max_idempotency_records` | `u32` | `10000`; `100..1000000` | file | live |
| `idempotency_ttl_ms` | `u64` | `86400000`; bounded by operation class | file | live |
| `snapshot_publish_timeout_ms` | `u64` | `2000`; `100..30000` | file | live |
| `durability` | enum | `fsync_atomic`; fixed | none | reject |
| `migration_policy` | enum | `verified_or_quarantine`; fixed | none | reject |

The journal path is derived under the owned data root and cannot be redirected to create split-brain
state.

### 6.4 `[protocol]` — `search-provider-protocol`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `max_frame_bytes` | `u32` | `8388608`; configurable downward only | file, env, CLI | restart |
| `max_in_flight_per_connection` | `u16` | `32`; `1..32` baseline | file, env, CLI | live decrease; restart increase |
| `hello_timeout_ms` | `u64` | `5000`; `100..30000` | file | live |
| `request_deadline_cap_ms` | `u64` | `120000`; `100..600000` | file | live |
| `compression` | `bool` | `false`; fixed baseline | none | reject |
| `fragmented_messages` | `bool` | `false`; fixed baseline | none | reject |
| `pairing_required` | `bool` | `true`; fixed | none | reject |

Pipe/socket identity is derived from installation/binding state; a user string cannot create an
unauthenticated second endpoint.

### 6.5 `[source_admission]` — `search-source-admission`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `bootstrap_policy_profile` | `ProfileId` | `baseline-safe-v1` | file | security barrier |
| `max_file_bytes` | `u64` | `16777216`; may be lowered live | file | security barrier |
| `allow_generated` | `bool` | `false` | file | new policy + explicit reconcile |
| `allow_vendor` | `bool` | `false` | file | new policy + explicit reconcile |
| `allow_binary` | `bool` | `false` | file | new policy + explicit reconcile |

Known credential stores, private keys and unconditional safety denies remain deny-by-default. A config
change creates a policy revision; it is not an implicit membership mutation.

### 6.6 `[source_reader]` — `search-safe-reader`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `max_file_bytes` | `u64` | `16777216`; cannot exceed admission ceiling | file | live decrease; restart increase |
| `stable_read_attempts` | `u8` | `3`; `1..8` | file | live |
| `retry_delay_ms` | `u64` | `25`; `0..1000` | file | live |
| `git_network_allowed` | `bool` | `false`; fixed | none | reject |
| `execute_hooks` | `bool` | `false`; fixed | none | reject |
| `execute_filters` | `bool` | `false`; fixed | none | reject |

### 6.7 `[reconcile]` — `search-source-reconcile`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `inventory_interval_ms` | `u64` | `300000`; `10000..86400000` | file | live |
| `overflow_reconcile_budget_ms` | `u64` | `5000`; bounded | file | live |
| `max_roots_per_sweep` | `u32` | `64`; `1..4096` | file | live |
| `max_entries_per_slice` | `u32` | `10000`; bounded | file | live |

No interval or budget allows a gap to be called current.

### 6.8 `[revision_store]` — `search-revision-store`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `max_total_bytes` | `u64` | `21474836480`; local quota | file | live decrease after safe plan |
| `max_single_object_bytes` | `u64` | `16777216`; must cover admitted ceiling | file | restart |
| `fsync_before_publish` | `bool` | `true`; fixed | none | reject |
| `verify_after_reopen` | `bool` | `true`; fixed | none | reject |
| `temporary_subdir` | relative component | `tmp`; fixed under data root | file | restart |

Lowering quota does not delete reachable objects immediately; retention plans a bounded mark/sweep.

### 6.9 `[lexical]` — `search-lexical`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `profile_id` | `ProfileId` | qualified baseline ID | file | new collection generation |
| `code_profile_enabled` | `bool` | enabled in CODE profile | file | new generation |
| `neutral_text_profile_enabled` | `bool` | enabled in LEXICAL profile | file | new generation |
| `collision_threshold_profile` | `ProfileId` | qualified fixture ID | file | new generation |

Tokenizer, hash, weighting, BM25 semantics and stopword/stemming behavior are identified by the profile;
they are not independent ad-hoc settings.

### 6.10 `[qdrant_process]` — `search-qdrant-supervisor`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `enabled` | `bool` | false in DIRECT; true in indexed profiles | file, CLI | dependency restart |
| `artifact_path` | local absolute path | required when enabled | file | dependency restart |
| `expected_sha256` | `Sha256Digest32` | required when enabled | file | dependency restart |
| `expected_version` | bounded text | required when enabled | file | dependency restart |
| `api_secret_ref` | `SecretRef` | required when enabled | file, whitelisted env | dependency restart |
| `port` | `u16` | `0` for supervised selection; loopback only | file | dependency restart |
| `startup_timeout_ms` | `u64` | `30000`; bounded | file | live for next start |
| `health_timeout_ms` | `u64` | `2000`; bounded | file | live |
| `restart_max_attempts` | `u8` | `3`; `0..10` | file | live |
| `restart_window_ms` | `u64` | `300000`; bounded | file | live |
| `auto_download` | `bool` | `false`; fixed | none | reject |
| `auto_upgrade` | `bool` | `false`; fixed | none | reject |

`artifact_path` and absolute paths are redacted by digest/location class in ordinary diagnostics.

### 6.11 `[qdrant_data]` — `search-qdrant-bridge`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `request_timeout_ms` | `u64` | `5000`; bounded | file | live |
| `upsert_batch_points` | `u32` | `256`; bounded by bytes | file | live |
| `delete_batch_points` | `u32` | `512`; bounded by bytes | file | live |
| `readback_batch_points` | `u32` | `256`; bounded | file | live |
| `strict_mode` | `bool` | `true`; fixed after index admission | none | reject |
| `wait_for_mutations` | `bool` | `true`; fixed correctness path | none | reject |
| `strong_ordering` | `bool` | `true`; fixed correctness path | none | reject |

Schema or capability-profile changes require a new collection generation even when transport tuning is
live.

### 6.12 `[index_reclaim]` — `search-index-reclaimer`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `batch_points` | `u32` | `512`; bounded | file | live |
| `max_batches_per_slice` | `u16` | `8`; bounded | file | live |
| `slice_budget_ms` | `u64` | `250`; bounded | file | live |

These settings cannot bypass committed exact manifests or pin watermarks.

### 6.13 `[query]` — `search-query-planner`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `default_budget_class` | `ProfileId` | `interactive-default-v1` | file, CLI | live |
| `max_scoring_legs` | `u32` | `16`; bounded | file | live |
| `max_prefetch_candidates_per_leg` | `u32` | `256`; bounded | file | live |
| `max_validated_candidates` | `u32` | `64`; bounded | file | live |
| `max_materialized_result_bytes` | `u64` | `1048576`; bounded | file | live |
| `fusion_profile_id` | `FusionProfileId` | qualified weighted-RRF baseline | file | rebuild/profile receipt |

Request grants may narrow budgets but cannot exceed server ceilings.

### 6.14 `[scheduler]` — `search-retrieval-executor`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `interactive_concurrency` | `u16` | `8`; bounded by host profile | file | live |
| `verification_concurrency` | `u16` | `2` | file | live |
| `background_concurrency` | `u16` | `2` | file | live |
| `interactive_queue` | `u32` | `64`; finite | file | live |
| `verification_queue` | `u32` | `16`; finite | file | live |
| `background_queue` | `u32` | `32`; finite | file | live |
| `background_pause_cpu_percent` | `u8` | `80`; `1..100` | file | live |
| `background_pause_memory_percent` | `u8` | `85`; `1..100` | file | live |

No zero/unbounded sentinel. Saturation remains a typed rejection or partial result.

### 6.15 `[overlay]` — `search-overlay`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `max_unsaved_sources_per_binding` | `u32` | `64`; finite | file | live |
| `max_unsaved_bytes_per_binding` | `u64` | `67108864`; finite | file | live |
| `unsaved_ttl_ms` | `u64` | `1800000`; finite | file | live restrictive |
| `saved_overlay_max_revisions` | `u32` | `1024`; finite | file | live |

Unsaved bytes remain excluded from durable stores regardless of configuration.

### 6.16 `[handles]` — `search-handles`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `ephemeral_ttl_ms` | `u64` | `600000`; finite | file | live restrictive |
| `max_ephemeral_per_binding` | `u32` | `256`; finite | file | live |
| `max_durable_per_binding` | `u32` | `128`; finite | file | live |
| `max_expansion_bytes` | `u64` | `1048576`; finite | file | live |
| `durable_unsaved_allowed` | `bool` | `false`; fixed | none | reject |

Lower TTL/quota changes may invalidate excess records with explicit receipts.

### 6.17 `[continuations]` — `search-continuation`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `ephemeral_ttl_ms` | `u64` | `300000`; finite | file | live restrictive |
| `max_per_binding` | `u32` | `64`; finite | file | live |
| `max_pinned_candidates` | `u32` | `512`; finite | file | live |
| `max_pin_ttl_ms` | `u64` | `300000`; finite | file | live restrictive |
| `durable_ordinary_queries` | `bool` | `false`; fixed | none | reject |

### 6.18 `[retention]` — `search-retention`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `sweep_interval_ms` | `u64` | `3600000`; bounded | file | live |
| `mark_batch_objects` | `u32` | `10000`; bounded | file | live |
| `delete_batch_objects` | `u32` | `1000`; bounded | file | live |
| `default_retention_ms` | `u64` | policy-defined finite value | file | security barrier |
| `claim_secure_erase` | `bool` | `false`; fixed | none | reject |

Legal holds, purge commands and tombstones are durable records, not editable config toggles.

### 6.19 `[observability]` — `search-eval`

| Field | Type | Default / bound | Override | Reload |
|---|---|---|---|---|
| `level` | enum | `info` | file, env, CLI | live |
| `rotation_bytes` | `u64` | `10485760`; bounded | file | live |
| `rotation_files` | `u16` | `5`; bounded | file | live |
| `include_query_text` | `bool` | `false`; fixed baseline | none | reject |
| `include_source_content` | `bool` | `false`; fixed | none | reject |
| `include_absolute_paths` | `bool` | `false`; fixed default diagnostics | none | reject |
| `privileged_debug_enabled` | `bool` | `false` | explicit CLI + binding authorization | security barrier |
| `privileged_debug_ttl_ms` | `u64` | `300000`; finite | explicit CLI | live restrictive |

Privileged debug remains binding-scoped, access-filtered and content-minimized; it is not a global
plaintext logging switch.

### 6.20 `[optional_profiles]` — `eliot-searchd`

```yaml
semantic: false
document: false
model_provider_profile: ProfileId | null
document_provider_profile: ProfileId | null
```

Any true/enabled value is `GATE_REQUIRED`. Configuration is rejected unless accepted P15/G5 evidence,
a dedicated ADR, exact provider/artifact qualification and the corresponding compiled feature exist.

## 7. Package section API

A package that owns a section provides behavior equivalent to:

```text
section_descriptor() -> ConfigSectionDescriptor
compiled_defaults() -> ConfigSectionInput
validate_section(input, platform, accepted_capabilities) -> ValidatedSection
section_digest(validated) -> Blake3Digest32
plan_section_change(old, new) -> SectionReloadDecision
apply_live_change(old, new, context) -> Result<ConfigApplyReceipt, PackageError>
```

Only `apply_live_change` is implemented for fields whose final action is `APPLY_LIVE`. Other actions are
executed through daemon lifecycle, security, generation or rebuild orchestration. Package validation
must reject fields it does not own.

## 8. Reconfiguration execution order

The daemon executes an accepted plan in this order:

1. stop or narrow request admission when required;
2. acquire security/lifecycle barriers;
3. cancel or drain affected work within bounded time;
4. persist required policy/config generation records without secret plaintext;
5. restart dependencies or create/rebuild a collection generation;
6. rerun qualification and readback checks;
7. publish immutable capability/settings snapshots;
8. invalidate affected handles, continuations, plans and caches;
9. publish readiness and the effective config fingerprint;
10. acknowledge only after every required receipt exists.

Failure before acknowledgement leaves the old snapshot active or the affected capability explicitly
fail-closed/quarantined. Mixed partial settings are never reported as effective.

## 9. Redaction

Default diagnostics may include:

- config schema and fingerprint;
- section names, owner packages and reload classes;
- scalar bounds and whether a value differs from default;
- opaque artifact/profile/receipt digests;
- path location class and path digest.

They must not include:

- `SecretRef` resolution or secret contents;
- source/query text;
- absolute paths by default;
- authorized corpus display names;
- raw environment values;
- provider-native configuration objects.

## 10. Required cross-package fixtures

```text
same_layers_same_fingerprint
unknown_section_and_key_fail_closed
duplicate_key_rejected
fixed_security_floor_cannot_be_overridden
plaintext_secret_rejected_in_every_layer
environment_and_cli_allowlist_enforced
package_section_owner_collision_rejected
live_change_is_atomic_snapshot_swap
security_change_uses_barrier_and_invalidation
lexical_or_schema_change_requires_new_generation
optional_profile_requires_gate_adr_artifact_and_feature
failed_reconfiguration_never_publishes_mixed_snapshot
redacted_effective_config_contains_no_secret_or_raw_path
```
