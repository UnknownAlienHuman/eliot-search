# search-config — typed settings, not a configuration operating system

Configuration exists to construct real runtime owners with understandable values. It is not a registry of
capabilities, a canonical identity, a reconfiguration planner or a readiness authority.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), local [source map](FUNCTIONS.md) and
[configuration guide](../../docs/config/README.md).

## Verified public-main scale and behavior

`search-config/src` contains about **158 KiB of production Rust** before `identity_tests.rs` (~15.1 KiB).
The daemon's `config_composition` adds about **50 KiB of production Rust** before its tests. Together they
implement:

- a custom restricted TOML event receiver and generic document tree;
- dynamic section/field descriptors and registry digests;
- generic layered values and source provenance;
- canonical section/snapshot encodings and fingerprints;
- generic diff/action DAG/receipt planning;
- redacted full-snapshot projection;
- capability/readiness booleans gated by accepted receipts.

Startup still fails the basic job. `effective_from_process` builds defaults, builds a candidate, then calls
`try_activate` with no receipt kinds. A valid `query.limit=11` is classified `ApplyLive` and refused as
`DAEMON_CONFIG_PARTIAL_REFUSED`.

`EffectiveDaemonConfig` stores the generic snapshot/registry plus one separately validated secret section.
Root locator hints are extracted by string-key lookup. Most registered fields—control migration epoch,
admission policy, lexical profile, query limit and optional semantic flag—are not represented as typed values
owned by their actual runtime consumers. Registration and fingerprints therefore describe an intended system
without constructing it.

Readiness adds another false authority: even when stores/Qdrant are observed live, search/indexed availability
requires caller-supplied `AcceptedReceipts` booleans and the status output verifies a configuration fingerprint.
A receipt cannot make an unconnected handler ready.

## Target settings

Use one concrete settings value containing only supported fields, for example:

```text
DaemonSettings {
  data_root: PathBuf
  source_roots: Vec<PathBuf>
  storage: StorageSettings
  qdrant: QdrantSettings
  query: QuerySettings
  lexical: LexicalSettings
  optional: selected real optional profile settings
}
```

Exact fields follow real consumers; do not add a field because a historical section packet names it.
Use nested Serde structs with `deny_unknown_fields`, optional layer values where absence matters, explicit
bounds and defaults in code.

Startup:

```text
capture one invocation
→ bounded read selected TOML
→ typed decode
→ merge selected env/CLI overrides over file/defaults
→ validate concrete values
→ construct owners/handlers
→ retain one applied DaemonSettings
```

No defaults-only runtime exists before this sequence. No `try_activate`, receipt set, fingerprint comparison
or capability gate participates.

## Input capture

Keep acquisition at the daemon/CLI edge:

- parse `args_os()` once;
- keep filesystem paths as `OsString`/`PathBuf`;
- use `vars_os()` and convert only selected `ELIOT_SEARCH__*` textual values;
- open one file once and read at most `max_bytes + 1` before decoding;
- reject unknown fields, invalid selected values and oversized collections;
- preserve source labels only for useful errors, not a cryptographic provenance transcript.

The locked `toml 1.1.7` + Serde decoder is acceptable for a small bounded local settings file. It builds an
intermediate tree, so the byte bound remains mandatory. Do not recreate `ConfigDocument`, custom TOML grammar
or generic `ConfigValue` around it.

## Runtime changes

Hot reload is not a generic feature. Support only concrete operations:

- a request default may update the direct query owner for future requests;
- static process/storage/endpoint settings require restart;
- retrieval-profile/schema changes use the normal publication/generation mechanism;
- selected-source changes use source reconciliation/publication;
- unsupported changes return restart-required/unsupported without mutating current settings.

Persist only a small current applied value if restart semantics actually require it. Use one typed redb row/
transaction; no completed-operation history, config intent, effect DAG, readback proof or second recovery
engine. External Qdrant work remains publication's responsibility.

## Status and readiness

Status reads the retained running `DaemonSettings` and actual owner state. It does not reread process inputs,
reconstruct a registry, hash the configuration or activate another snapshot.

Readiness is derived directly from constructed/observed components:

```text
root/control/object owners open
source inventory/preparation state
Qdrant connection + selected generation schema
current published view
actual Search/Exact/Continue/Expand handlers
```

Remove `AcceptedReceipts`, optional gate booleans and config-fingerprint authority. An unavailable optional
handler is reported unsupported; it does not make lexical search unavailable.

## Removal boundary

After consumers move, delete:

- custom TOML event receiver, `ConfigDocument`, generic builder/syntax/decode tree;
- `ConfigRegistry`, section/field descriptors, owner names and registry digests;
- generic `ConfigValue`, layer operations, project/merge machinery not needed by typed overrides;
- canonical identity/fingerprint/snapshot encoders;
- generic diff, action lattice, reconfiguration plan and `ReceiptKind`;
- full redacted snapshot/status machinery;
- daemon registry/snapshot/activation/readiness acceptance receipts;
- schema/profile migration compatibility and source/golden identity fixtures.

Keep only concrete settings, small merge/validation helpers, bounded input capture and direct status rendering.
If those fit clearly in daemon composition, delete this crate instead of preserving a wrapper boundary.

## Completion

A non-default query limit reaches the real query handler; selected source/Qdrant/storage values reach their
owners; unknown/invalid values fail before side effects; non-Unicode unrelated environment entries cannot
panic startup; file size is bounded before allocation; status reports actual running settings/components; and
no registry/fingerprint/receipt platform remains reachable.
