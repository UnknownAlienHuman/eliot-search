# Configuration TOML: existing format and simpler replacement

Read the [implementation map](FUNCTIONS.md). `eliot-config-toml@2` describes the current Rust parser;
it does not freeze a pre-release grammar or require a compatibility/migration subsystem.

## Existing decoder

`parse_document` uses the locked `toml_parser =1.1.4` event API and Search's `DocumentBuilder`.
It requires UTF-8, no BOM, root `schema_version=2` and `profile` before flat bare-name tables. Values
are booleans, decimal i64, single-line strings and flat string arrays. Dotted/quoted keys, nested/inline/
array tables, multiline strings, non-decimal numbers, floats and datetimes are rejected. Schema1 is
unsupported. These are current implementation restrictions, not product requirements.

Current W1 limits include a 1 MiB input, 16,384 logical lines, 131,072 tokens, 4 MiB logical token
storage, 256 sections and 8,192 entries. Additional identifier/value/list and canonical-CBOR ceilings
apply. They describe this adapter's accounting; they are not measured process memory or I/O latency.
Do not carry the full budget taxonomy into a finite settings decoder without a real requirement.

## Use the existing high-level donor

`Cargo.lock` already contains `toml 1.1.7+spec-1.1.0`; `xtask` uses its `std,serde,parse` features.
The production config package currently depends only on `toml_parser`, so the direct typed decoder
still needs integration. This is not a version-upgrade proposal.

- [Public decoding API](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/mod.rs): `toml::from_slice::<FileSettings>` / `from_str` with Serde derives.
- [Deserializer](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/deserializer/mod.rs): parses a `DeTable` before visiting the target struct.
- [Parser](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/parser/mod.rs): allocates token/event vectors and a document tree; its default recursion limit is 80. Keep `unbounded` disabled.

The tag `toml-v1.1.7` resolves to this inspected commit. The donor is **not streaming** and typed
output does not prevent its intermediate allocations. Bound file bytes before decoding, keep concrete
field/list limits and inspect the bounded worst case during integration. For a small local settings
file, this documented overhead is a reasonable trade for removing the custom receiver, double string
decoding and generic document machinery. No memory benchmark was run in this documentation pass.

Decode into the actual settings fields, merge optional overrides over defaults, reject unknown fields
and validate values before owner construction. Use ordinary TOML syntax supported by the donor and
the selected types. No legacy fingerprint restoration, schema migration table, syntax receipt or
per-file cryptographic provenance is required. A clear unsupported-format error leaves the original
file unchanged.
