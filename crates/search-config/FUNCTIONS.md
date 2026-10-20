# search-config — source audit and direct replacement

## Gross source inventory

Public main `c8e38b6`:

| Area | Gross production source before tests |
|---|---:|
| `search-config/src` | ~158.0 KiB |
| daemon `config_composition/kernel*` | ~50.0 KiB |
| combined | ~208 KiB |

`identity_tests.rs` adds ~15.1 KiB and daemon config tests add substantially more. This is not a promised net
deletion; concrete settings, bounds and input capture remain. It shows the cost of preserving a generic
platform around a small local settings model.

## Current package modules

| Module | Current behavior | Replacement |
|---|---|---|
| `document`, `builder`, `decode`, `syntax` | Restricted TOML event receiver, double-pass string decode, generic `ConfigDocument`, section/entry accounting. | Bound file bytes once and decode concrete Serde settings with locked `toml`. |
| `registry`, `section`, `value`, `names` | Dynamic field/section descriptors, generic values, owner names, security floors, allowed sources, action sets. | Concrete Rust fields, direct defaults/range validation and selected override policy. |
| `merge`, `builder`, `environment` | Generic file/env/CLI layer operations and provenance. | Small optional-layer merge over real setting structs. |
| `canonical_identity`, `fingerprint`, `snapshot` | Canonical CBOR-like snapshot identity and BLAKE3 fingerprint used by receipts/status. | Ordinary typed equality/current settings revision only when a real owner needs it. |
| `diff`, `action`, `plan` | Generic action lattice and ordered receipt requirements across capabilities. | Direct live update, explicit restart or publication generation switch. |
| `redact` | Projects the complete generic snapshot according to field policies. | Serialize a small selected redacted status struct from actual running state. |
| `limits`, `error` | Very broad parser/registry/canonical limits and closed errors. | Retain only file/field/list/value/output bounds and useful errors. |

## Current daemon composition

| File | Current behavior / defect |
|---|---|
| `process.rs` | Parses config flags twice (`String` and `OsString` variants), rejects non-UTF8 config paths, uses `std::env::vars()` before prefix filtering and performs metadata check → unbounded `fs::read` → length check. |
| `registry.rs` | Registers instance, mandatory secrets, control migration, source admission, lexical, query, optional semantic and source-root fields with owner/digest/action metadata. |
| `snapshot.rs` | Reconstructs the registry, merges/project-validates every section, requires `ValidatedSecretsConfig`, builds full fingerprinted snapshot. It retains no typed query/lexical/control settings fields. |
| `activation.rs` | Diffs fingerprints, builds action/receipt plan and refuses candidate when receipt enum values are absent. It executes/observes none of those effects. |
| `readiness.rs` | Gates actual search/indexed availability on caller-supplied `AcceptedReceipts` booleans and optional gate acceptance; returns a configuration fingerprint as readiness evidence. |
| `root_locators.rs` | String-key projection from generic snapshot into one actually consumed setting. Useful behavior moves to typed settings. |
| status/diagnostics | Rebuilds redacted generic view and manually formats fingerprint-heavy JSON rather than serializing actual running component state. |

## Confirmed startup failure

`query.limit` is registered with default 10 and `ApplyLive`. Startup builds defaults and a candidate, then:

```text
try_activate(defaults, candidate, empty_receipt_set)
```

A valid non-default limit therefore returns `DAEMON_CONFIG_PARTIAL_REFUSED`. The fixture
`startup_partial_refuses_and_retains_defaults_without_leaks` locks in this behavior. Adding a `LiveApply`
enum value would only bypass the gate; it would still not pass the value to the direct query handler.

## Current field disposition

| Registered field/family | Target |
|---|---|
| `instance.mode` | Remove if only `standalone` exists; otherwise a direct closed startup enum. |
| `instance.data_root` | Direct `PathBuf`, captured once. |
| `[secrets]` backend/lease/rotation/Qdrant ref | Remove generic section; storage/Qdrant/restricted-client own concrete values. |
| `control.durability` / `migration_epoch` | Remove pre-release migration/fixed receipt semantics; current root format/durability is selected directly. |
| `source_admission.allow_generated` | Keep only if the simplified policy evaluator actually consumes it; otherwise delete. |
| `lexical.profile_id` | Direct retrieval profile setting; change requires normal generation/publication. |
| `query.limit` | Direct bounded default used by the query owner for new requests. |
| `optional_profiles.semantic` | Do not gate lexical readiness. Keep only when a real semantic handler/profile is selected. |
| `source_root_locators.entries` | Direct bounded `Vec<PathBuf>` used by inventory/reconcile. |
| Qdrant/process settings | Direct endpoint/optional managed profile in `[qdrant]`. |
| storage protection | Direct root storage profile in `[storage]`. |

Do not preserve unused fields for documentation parity.

## Target structures and merge

Suggested shape; exact ownership follows final consumers:

```text
FileSettings {
  instance?: InstanceLayer
  storage?: StorageLayer
  qdrant?: QdrantLayer
  source?: SourceLayer
  lexical?: LexicalLayer
  query?: QueryLayer
}

DaemonSettings {
  data_root: PathBuf
  storage: StorageSettings
  qdrant: QdrantSettings
  source_roots: Vec<PathBuf>
  source_policy: SourcePolicySettings
  lexical: LexicalSettings
  query: QuerySettings
}
```

Use `#[serde(deny_unknown_fields)]`. File/env/CLI layers contain `Option` fields. Merge in the supported
precedence, then validate final values once. CLI/env override parsing is field-specific and small; do not
reconstruct a generic dotted-key database.

Paths stay `OsString`/`PathBuf`. Text enums/integers are converted only after selecting their known field.
Secret bytes are not ordinary config values.

## Capture implementation

```text
parse args_os once
→ open file once with no-follow/regular-file policy selected by application
→ read at most max_file_bytes + 1
→ toml::from_slice<FileSettings>
→ vars_os, select known prefix/field, convert selected text only
→ parse known CLI overrides
→ merge + validate DaemonSettings
```

`Read::take` bounds bytes but not waiting time; local regular-file capture may remain synchronous under the
startup bound. Do not add a source-custody protocol for configuration.

## Startup and status

Construct each owner with its direct settings. Retain `Arc<DaemonSettings>` or equivalent current value for
status/new request defaults. If a value is persisted, use one typed current redb row only when restart must
remember a runtime-applied change; process file/env/CLI inputs normally remain the startup source.

Readiness is factual:

```text
owner/root/control/object ready?
Qdrant connected and selected generation verified?
current PublishedView exists?
handler constructed?
```

No accepted receipt booleans, qualification digest or configuration fingerprint can turn false into true.
An optional unavailable handler does not disable baseline Search.

## Runtime change semantics

- `query.limit`: optional direct atomic/current setting for future requests;
- source list/policy: run actual reconcile/publication operation or require restart;
- Qdrant endpoint, root/storage profile, process mode: restart;
- lexical/model/profile/schema: new generation/publication;
- unsupported field: reject before changing retained settings.

Only publish a new applied settings value after the concrete operation succeeds. Possible Qdrant effects use
publication's pending artifact; there is no config transaction journal.

## Delete after cutover

- all generic document/parser/builder/value/registry/section/snapshot/fingerprint/diff/action/plan/redact APIs;
- daemon registry, `EffectiveDaemonConfig` generic snapshot, `try_activate`, receipt sets and accepted readiness receipts;
- generic config source digests/provenance and schema-profile compatibility;
- full snapshot/manual status JSON and identity/golden fixtures;
- historical section packets/fields with no runtime consumer.

If typed decode/merge/validation is small, move it into daemon/config owner and remove the crate rather than
leave a generic facade.

## Focused checks

- non-default values reach their actual owners;
- file < env < CLI precedence for supported fields;
- unknown/duplicate/type/range-invalid fields fail before owner construction;
- unrelated non-Unicode environment entry cannot panic;
- non-UTF8 config/root paths work where OS supports them;
- growing/oversized file cannot exceed allocation ceiling;
- status reflects retained runtime values and actual readiness;
- optional semantic absence does not block lexical Search;
- supported live update and restart-required change are distinct;
- no fingerprint/receipt can claim application of an unperformed effect.
