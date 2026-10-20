# Configuration TOML — one bounded typed file

The old `eliot-config-toml@2` grammar is an implementation artifact, not a compatibility contract. Replace its
custom event receiver and generic document builder with the locked high-level TOML/Serde decoder over a small
bounded file.

Read the [source audit](FUNCTIONS.md) and [configuration guide](../../docs/config/README.md).

## Current decoder cost

The existing path uses `toml_parser` events, custom syntax callbacks, a double-pass string decoder, generic
`ConfigDocument`, layer operations, dynamic section/field descriptors and later canonicalization. It requires:

- UTF-8 and no BOM;
- root `schema_version=2` and `profile` before flat tables;
- flat bare-name sections/keys;
- booleans, decimal i64, single-line strings and flat string lists only;
- rejection of ordinary TOML dotted/quoted/nested/inline/array tables, multiline strings, floats/datetimes and
  non-decimal integers.

Its W1 budget vocabulary allows up to 1 MiB, 16,384 lines, 131,072 tokens, 4 MiB logical token storage, 256
sections and 8,192 entries plus additional canonical-CBOR limits. These ceilings describe the generic adapter,
not a measured need for local settings.

## Selected donor

`Cargo.lock` already includes `toml 1.1.7+spec-1.1.0`; `xtask` uses its `std,serde,parse` features. Production
`search-config` currently depends only on `toml_parser`, so direct `toml` and `serde` dependencies must be
added to the actual owner during implementation.

Inspected donor:

- [`toml::from_slice`](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/mod.rs)
- [deserializer](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/deserializer/mod.rs)
- [parser](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/parser/mod.rs)

The donor builds token/event/document structures before visiting the target type. It is not streaming. Keep a
small file-byte bound and the default recursion guard; do not enable `unbounded`. For local settings this is a
reasonable trade for deleting the custom grammar and generic configuration OS. No memory benchmark is claimed.

## Target file

The exact supported fields follow actual owners. Illustrative shape:

```toml
[instance]
data_root = "..."

[storage]
protection = "plain"

[source]
roots = ["..."]
allow_generated = true

[qdrant]
mode = "external"
endpoint = "http://127.0.0.1:6333"
connect_timeout_ms = 5000

[lexical]
profile = "qdrant-bm25-v1"

[query]
default_limit = 10
max_page_size = 50
```

Optional managed Qdrant or model/document profiles add only fields their selected implementation consumes.
There is no universal `[secrets]`, control migration epoch, gate acceptance or capability registry section.

Use nested structs with `#[serde(deny_unknown_fields)]`. File layer fields are `Option<T>` where absence means
use default/another override. Validate after merge:

- paths/URLs and closed enums;
- integer/item/string bounds;
- source root count and duplicates/nesting policy;
- timeout/output ceilings;
- selected profile compatibility with constructed owners.

Do not expose plaintext credentials in the ordinary file. Concrete local secret sources supply the optional
Qdrant/restricted/storage credential directly to their owners.

## Capture and bounds

```text
open selected file once
→ verify selected regular/no-follow policy on that object
→ read at most max_file_bytes + 1
→ reject overflow
→ toml::from_slice<FileSettings>
→ validate typed field/list bounds
```

A practical default may be tens or hundreds of KiB, not the old 1 MiB/131k-token grammar allowance; choose
from actual documented settings and tests. `Read::take` limits bytes, not wait time. Startup file reads may be
synchronous for a selected local regular file.

Use `vars_os()` for environment capture and convert only known selected textual values. Keep config/data/source
paths as `PathBuf` through CLI/file ownership. CLI/env override syntax may remain small and field-specific;
do not reconstruct arbitrary dotted-key TOML or a generic map.

## No compatibility platform

This is a pre-release configuration. Unsupported old files fail clearly and remain unchanged. Do not add:

- schema1/schema2 migration;
- old profile/fingerprint restoration;
- custom canonical ordering/bytes;
- section descriptors or ownership digests;
- syntax/provenance receipts;
- duplicated parser for env/CLI;
- a legacy decoder fallback.

Change the actual CLI/daemon reader and current configuration file/examples together.

## Focused verification

- defaults and supported file/env/CLI precedence;
- ordinary TOML strings/arrays used by supported fields;
- unknown/duplicate/type/range-invalid fields;
- oversized/deep/large-list input before unbounded retention;
- non-Unicode unrelated environment and OS paths;
- no plaintext credential in parsed/serialized diagnostics;
- every documented field reaches a real consumer.
