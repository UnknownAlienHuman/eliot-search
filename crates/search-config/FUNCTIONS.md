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
| `document`, `builder`, `decode`, `syntax` | Restricted TOML event receiver, double-pass string decode, generic `ConfigDocument`, section/entry accounting. | Bound file bytes once and decode concrete settings with the selected high-level `toml`/Serde donor. |
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
| `readiness.rs` | Gates actual search/indexed availability on caller-supplied `AcceptedReceipts` booleans; `optional_profiles.semantic=true` requires optional gate acceptance even without a semantic handler. Returns a configuration fingerprint as readiness evidence. |
| `root_locators.rs` | Projects `source_root_locators.entries` into bounded `RootBindingId -> PathBuf` hints used by DIRECT commands. Useful behavior moves to typed settings. |
| status/diagnostics | Rebuilds redacted generic view and manually formats fingerprint-heavy JSON rather than serializing actual running component state. |

## Observed callers and startup refusal

Caller search at `5af8cde5` distinguishes three paths:

| Actual consumer | What is consumed | What this does not establish |
|---|---|---|
| `app/kernel/status.rs` and `public_runtime_service/kernel/runtime.rs` → `effective_from_process` | Merged config for shell/DIRECT service startup, readiness and diagnostics. Invalid/refused config can stop startup. | `query.limit`, lexical/profile, root and admission declarations are not passed to their operational handlers. A status field is not an applied setting. |
| `owner_composition/kernel/direct_registry_command.rs::capture_hints` → `RootLocatorHints`, then `fresh_root.rs::requalify_roots` | Bounded `RootBindingId -> PathBuf` locator hints for already selected/admitted roots. This input accepts only `source_root_locators`. | A locator does not admit a new member, set source policy or prove a complete inventory. Preserve this real consumer during cutover. |
| `owner_composition/kernel/native_dek_selection.rs::capture_candidate` → `RevisionDekSelectionPolicy::from_config` | The explicit DEK-selection command checks the backend and fingerprint tag, then binds the fingerprint and secrets-section digest. | It does not apply lease TTL, rotation grace or the Qdrant secret reference. Change this narrow caller with its actual object/key owner; do not delete shared protection code as provider-only machinery. |

These paths are source callers, not proof of successful command routing. The DIRECT service's serve command
is shadowed by the earlier registry selector in the [daemon entry chain](../../bins/eliot-searchd/README.md#exact-production-entrypoint-order).
None was executed here. `derive_readiness` also hardcodes
`configuration_ready` and `endpoint_ready` to true; neither proves an opened root or a listening endpoint.
The replacement below constructs owners from actual settings and reports observations of those owners.

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
| `source_root_locators.entries` | Preserve the existing bounded ID-to-path lookup while its DIRECT consumer remains. Keep relocation hints separate from an explicit source selection/admission operation; do not treat them as corpus membership. |
| Qdrant/process settings | Direct endpoint/optional managed profile in `[qdrant]`. |
| storage protection | Direct root storage profile in `[storage]`. |

The table describes target disposition, not applied runtime settings. Do not preserve unused fields for
documentation parity or turn the legacy locator map into a new membership authority.

## Target structures and merge

The inspected lock at `5af8cde5` contains `toml_parser 1.1.4+spec-1.1.0`, used by the custom decoder.
It contains neither `toml` nor Serde. High-level typed TOML is the replacement choice, not an installed API;
pin its compatible version/features and regenerate the lock with the consuming code change. See the
[donor register](../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

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

### Sparse layers and validation

This is the selected replacement policy, not the legacy parser's behavior or a shipped file schema.
Decode closed nested structs with `#[serde(deny_unknown_fields)]` on **each** struct; the attribute on the
outer struct does not close its nested fields. Do not combine it with `flatten`. Every layer field is optional,
including fields inside an optional section. An absent section and a present empty section both inherit.
Serde defaults, if used for layer structs, produce only absent fields; application defaults are applied once
to the final settings, not inserted into each incoming layer. See the [Serde attributes](https://serde.rs/container-attrs.html)
and [field defaults](https://serde.rs/field-attrs.html), checked 2026-10-23.

Apply defaults < file < selected environment < CLI, field by field. Omission inherits; `false`, `0` and an
empty list are explicit values. A supplied list replaces the lower-precedence list in full. Do not merge
lists by position, append implicitly or reinterpret an empty source list as all sources. Repeated scalar
CLI options, including aliases for the same field, are errors. A selected repeated list-item option builds
one bounded CLI list; it is not a sequence of appends to file settings. Exact flag names ship with the parser.

Decode every supplied layer before constructing owners. Invalid UTF-8 file content, malformed TOML, unknown
nested keys, duplicate definitions, wrong types and integers outside the chosen Rust representation fail even when a higher layer
would override them. TOML duplicate-definition rejection belongs to the parser, not `deny_unknown_fields`.
After merging, validate domain ranges, cross-field constraints and checked unit conversions once on the
effective value. Do not use truncating casts or unchecked duration/size arithmetic. Bound captured file/value
bytes, selected entry counts and collected CLI lists before decode/merge; an override cannot excuse oversized
input. The small file cap bounds TOML input, not an exact parser heap or CPU allowance.

Ignore unrelated environment entries without decoding their values. Reject unknown names within the selected
`ELIOT_SEARCH__` namespace, including malformed names, so misspelled settings do not silently disappear.
Only known text fields require Unicode conversion; path fields retain their OS values.

Examples of that policy, independent of the final field/flag spelling:

| Inputs | Effective result |
|---|---|
| Default limit 10; file 20; environment 30; CLI 40 | 40. |
| File limit 20; a higher layer contains an empty query section | 20; omitted fields do not reset lower values. |
| File list `[A, B]`; higher layer `[]` | Empty selection; no append or wildcard. |
| File Boolean `true`; higher layer `false` | `false`, not absence. |
| Representable limit 0 in file; CLI 5; final minimum 1 | 5 is valid; without the override, reject 0. |
| Wrong-type or unrepresentable file limit; valid CLI limit | Reject the malformed lower layer. |

### Paths and diagnostics

Keep config-file locators and supported path overrides from `args_os`/`vars_os` as `OsString`/`PathBuf`.
Convert only selected textual fields. TOML file contents and string paths are Unicode: they cannot encode
arbitrary Unix path bytes or Windows unpaired surrogates. The baseline uses Unicode paths in TOML and OS-valued
CLI/environment path inputs where exposed; no lossy conversion or invented byte-escape format. This distinction
also applies to retained locator hints. A list of OS paths must not be split on a delimiter that can occur in
a pathname. Reject NUL/invalid OS paths at the actual path owner. See [TOML 1.1.0](https://toml.io/en/v1.1.0).

The [inspected TOML error API](https://docs.rs/toml/latest/toml/de/struct.Error.html) is
`1.1.8+spec-1.1.0` as of 2026-10-23. `span()` is optional and indexes the original document. Use a valid
span to report bounded location information; otherwise report the known field/source label and error class.
Do not unwrap the span, expose raw file/value excerpts through automatic `Display`/`Debug`, or dump argv/environment: parse
errors may contain a mistakenly supplied credential. Secret bytes are not ordinary config values.

## Capture implementation

```text
capture cwd; parse args_os once into config locator and typed CLI layer
→ open selected file once; verify the opened handle is a regular file
→ read at most max_file_bytes + 1
→ reject invalid UTF-8, then toml::from_str<FileSettings>
→ vars_os, check selected namespace/field, build typed environment layer
→ merge + validate DaemonSettings
```

`Read::take` bounds bytes but not waiting time; local regular-file capture may remain synchronous under the
startup bound. Do not add a source-custody protocol for configuration.

Select the high-level `toml` `parse`/`serde` features and Serde derive with the actual code/lock change;
the [verified decoder API](https://docs.rs/toml/latest/toml/fn.from_str.html) was
`1.1.8+spec-1.1.0` on 2026-10-23. Check `max_file_bytes + 1` for overflow. Configuration lookup has no implicit
current-directory/parent search. An explicitly named missing or unreadable file is an error; when no file is
selected, use defaults plus selected overrides. Resolve a relative config-file locator against captured startup
cwd first. Relative TOML paths use that selected absolute config pathname's parent; CLI/environment paths use
captured startup cwd. Resolve each layer's paths before merging; a later cwd change cannot retarget a setting.
An optional future fixed application config location must be explicit in its parser/help.

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

## Retired repository catalogues

Removed catalogues do not define a supported schema. Publish a small runnable example only with its connected
typed settings consumers; do not restore wave/section registries or no-op examples for old fixtures. The
[R029 record](../../docs/audit/ANALYSIS_REGISTRY.md#r029-configuration-catalogue-removal-and-typed-input-policy)
owns removal evidence. Storage lifetime remains with [stopped-root maintenance](../search-runtime/search-retention/FUNCTIONS.md).

## Focused checks

- non-default values reach their actual owners;
- file < env < CLI precedence for supported fields;
- nested unknown/duplicate/type-invalid fields fail before owner construction; final domain bounds follow the layer examples;
- unrelated non-Unicode environment entry cannot panic;
- non-Unicode OS path inputs survive capture; TOML paths obey the stated Unicode/relative-base policy;
- growing/oversized file cannot exceed allocation ceiling;
- status reflects retained runtime values and actual readiness;
- optional semantic absence does not block lexical Search;
- supported live update and restart-required change are distinct;
- no fingerprint/receipt can claim application of an unperformed effect.
