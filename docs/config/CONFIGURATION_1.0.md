# Configuration source audit

This file was the Architecture 8.4 cross-package configuration catalogue. It is **not** the supported file
schema and must not be used to generate section APIs, reload receipts or capability gates.

Read the current [configuration guide](README.md),
[`search-config` source map](../../crates/search-config/FUNCTIONS.md) and
[typed TOML replacement](../../crates/search-config/FUNCTIONS.md#target-structures-and-merge).

## What the inspected source implements

Main `c8e38b6` supplied the original audit; the relevant public callers were rechecked at `5af8cde5`.
The [caller map](../../crates/search-config/FUNCTIONS.md#observed-callers-and-startup-refusal) distinguishes
startup/status, root relocation hints and the existing explicit DEK-selection command.

The current decoder accepts the custom `eliot-config-toml@2` shape:

```toml
schema_version = 2
profile = "direct"
```

followed by restricted flat tables. The daemon dynamically registers only:

| Section | Registered fields | Actual problem |
|---|---|---|
| `instance` | `mode`, `data_root` | generic snapshot; not a typed root owner constructed from settings |
| `secrets` | `backend`, `lease_ttl_ms`, `rotation_grace_ms`, `qdrant_api_secret_ref` | DEK selection checks the backend and binds fingerprint/section digest; TTL, grace and Qdrant reference are not applied by that caller |
| `control` | `durability`, `migration_epoch` | pre-release migration/action metadata |
| `source_admission` | `allow_generated` | registered value is not a typed setting of the real source path |
| `lexical` | `profile_id` | registered identity is not applied to a connected Qdrant generation |
| `query` | `limit` | non-default startup value is receipt-gated and does not reach the query handler |
| `optional_profiles` | `semantic` | readiness reads the flag and can block baseline Search when an optional receipt is absent; no semantic handler is constructed |
| `source_root_locators` | `entries` | string-projected ID-to-path hints reach DIRECT root requalification; they do not select or admit corpus members |

The example/reference previously advertised many additional sections—protocol, source reader, reconcile,
revision store, Qdrant data/process, reclaimer, scheduler, overlay, handles, continuations, retention and
observability. Most are not registered by the daemon and therefore are not a file-to-runtime API.

## Confirmed defects

- startup constructs defaults and a candidate, then calls `try_activate` with an empty receipt set;
- a valid non-default `query.limit` is classified `ApplyLive` and refused;
- bypassing that refusal still would not pass the value to the real query owner;
- `EffectiveDaemonConfig` retains a registry/fingerprinted generic snapshot rather than concrete settings;
- readiness depends on caller-supplied accepted-receipt booleans and a configuration fingerprint;
- environment capture uses `std::env::vars()` before filtering, so unrelated non-Unicode values can panic;
- `--config-file` is forced through UTF-8 although it is an OS path;
- metadata is checked before an unbounded `fs::read`, so replacement/growth can exceed the intended bound.

This is roughly 208 KiB production Rust before tests across `search-config` and daemon composition. It is a
removal target, not a base for another configuration framework.

## Target

```text
args_os once
+ one bounded typed TOML file
+ selected vars_os / field-specific CLI overrides
→ merge optional layers
→ validate one concrete DaemonSettings
→ construct actual owners and handlers
→ retain running settings for status/new requests
```

Illustrative shape; exact fields follow real consumers:

```text
DaemonSettings {
  data_root: PathBuf
  source_roots: Vec<PathBuf>
  storage: StorageSettings
  qdrant: QdrantSettings
  lexical: LexicalSettings
  query: QuerySettings
  selected optional settings only when their handlers exist
}
```

Select and pin the high-level TOML/Serde decoder with nested `deny_unknown_fields` structs during implementation.
The current lock contains only the low-level `toml_parser` donor; typed settings are not yet implemented.
Bound bytes before decoding. Keep paths as `PathBuf`; inspect only selected environment variables through
`vars_os()`. Plaintext credentials are not ordinary configuration values.

Startup is not a live update of a defaults-only runtime. Non-default valid values construct the owners
directly. A running change exists only when a real owner implements it:

- query defaults may update future requests;
- source selection/policy runs actual reconciliation/publication or requires restart;
- root/storage/Qdrant endpoint settings require restart/reopen;
- lexical/model/schema changes build and publish a coherent new generation;
- unsupported changes leave current settings untouched.

No dynamic section registry, canonical configuration identity, action DAG, `ReceiptKind`, operation journal,
profile migration or accepted-gate readiness remains.

## Section disposition

| Old packet | Disposition |
|---|---|
| instance | keep direct root path; keep a mode only if another real mode exists |
| secrets | replace with concrete storage/Qdrant/restricted-client inputs |
| control | remove public migration/journal configuration; current redb owner selects its format directly |
| protocol | current local framing owns finite limits; no universal pairing toggle |
| source admission/reader/reconcile | consolidate real source roots, policy and acquisition limits under the source path |
| revision store | current object owner owns storage profile and enforced limits |
| lexical | direct published retrieval profile; change through generation/publication |
| Qdrant process/data | one `[qdrant]` endpoint plus optional real managed-child settings |
| index reclaim/retention | remove generic platforms; publication cleanup and object GC own finite work |
| scheduler | private bounded semaphore/queue only if the real handler needs it |
| overlay | no settings until a real editor integration selects the thin memory-only overlay |
| handles/continuations | one temporary `QueryWindow` with enforced limits |
| observability | add only fields consumed by an actual logger/status owner |
| optional profiles | exact selected extractor/model settings; absence never blocks lexical Search |

## Current example

[`config/eliot-search.example.toml`](../../config/eliot-search.example.toml) is intentionally a legacy-parser
no-op. It proves only the root headers can be read. It is not a persistent Search setup and does not document
the target schema. #333 replaces it together with the parser and actual consumers.

## Completion

A non-default value must reach its real owner; file/env/CLI precedence and limits must be observed; unknown
input must fail before side effects; non-Unicode unrelated environment/path data must not panic; status must
report actual running settings/components; and no registry/fingerprint/receipt authority may remain reachable.
