# FILE profile `eliot-config-toml@2`

The sole current file grammar is schema2 `eliot-config-toml@2`. Its constants are
`CONFIG_TOML_SCHEMA_VERSION` and `CONFIG_TOML_PROFILE`. The composition `profile` selects an
externally authorized capability profile; it is a separate identity.

This is `VERSIONED_REBUILD`. Schema1 files return the closed `UnsupportedSchemaVersion` error
and require explicit migration. No legacy parser or automatic migration path remains. Typed
`ConfigDocument::from_entries` retains positive-schema admission and makes no TOML syntax claim;
the current daemon registry and its env/CLI producers use schema2. Merge rejects schema mismatch.

## Grammar and framing

`parse_document` accepts already captured FILE bytes and source metadata. It performs no I/O,
expansion or resource execution. Empty input, excessive raw bytes, BOM and invalid UTF-8 are rejected
before donor parsing. A local framing scan examines only CR/LF: LF and CRLF each terminate one line,
each terminator counts, a final unterminated segment counts once, and a trailing terminator adds no
empty final line. Lone CR anywhere is rejected. Byte and logical-line limits precede token storage.

Both root headers appear once before the first table:

```toml
schema_version = 2
profile = "direct"
```

Missing or unsupported schema gives `UnsupportedSchemaVersion`; missing/non-string profile gives
`ProfileNotAuthorized`. A repeated root header gives `DuplicateKey`. Root keys other than these two
give `UnknownKey`. Table names `profile` and `schema_version` conflict with root scalars and give
`DuplicateTable`. Inside an ordinary section, fields with those names are scoped fields; the registry
decides whether they are known. No syntax implicitly returns to root scope.

Only flat bare-name tables and bare keys are supported, subject to existing typed name validation.
Values are booleans, decimal i64 integers, single-line basic/literal strings and flat string arrays.
The donor validates whitespace, comments, escapes and decimal spelling. Decimal leading plus and
underscores between digits are supported; the normalized integer must fit i64. Arrays can span lines
and retain item order and trailing commas. Empty arrays and empty strings are supported.

Quoted/dotted keys, dotted/nested/array tables, inline tables, nested arrays, mixed/non-string arrays,
multiline strings, radix integers, floats and datetimes return closed errors. Key encodings and event
shapes drive rejection; Search does not scan quotes, assignments, arrays, escapes or number grammar.
Unknown section/field, secret and override policies remain owned by the existing registry/merge code.

## Explicit migration table

The historical column describes the old file parser's value/shape behavior under schema1.
It is source-reviewed compatibility inventory; fixture execution is deferred.

| Input property | Historical parser | Current schema2 profile |
| --- | --- | --- |
| schema1 file | Current format | `UnsupportedSchemaVersion` |
| flat bare tables/keys, booleans, decimal i64 | Accepted | Accepted |
| properly grouped decimal underscores | Accepted | Accepted |
| decimal leading plus | Rejected | Accepted |
| malformed `-_1` | Accepted as -1 | `UnsupportedSyntax` |
| single-line basic/literal strings | Accepted subset | Donor single-line grammar |
| Unicode basic-string escapes | Rejected | Accepted for valid scalar values |
| flat single-line string array with trailing comma | Accepted | Accepted |
| multiline flat string array | Rejected | Accepted |
| terminal lone CR after a complete value | Could be trimmed/accepted | `UnsupportedSyntax` |
| LF/CRLF and bounded final unterminated segment | Accepted | Accepted with explicit line counting |
| quoted/dotted keys, nested/inline/array tables | Rejected | Rejected |
| nested/mixed arrays, floats/datetimes, radix integers | Rejected | Rejected |

Alternative spellings can produce equal typed values. Equality identities still bind exact supplied
source/reset provenance: changing a captured file's source digest can change its section/snapshot
identity even when decoded values agree.

## Bounded construction

The private event receiver uses the same `DocumentBuilder` as typed entry admission. It retains one
field target and at most one flat string list, rather than a donor document tree. `ValidateWhitespace`
wraps the receiver; `RecursionGuard` at value depth1 rejects nested aggregates. Standard-table
callbacks do not consume value depth, so multiple flat tables remain supported.

W1 ceilings include:

| Population | Ceiling |
| --- | --- |
| Raw document / logical lines | 1 MiB / 16,384 |
| Lexer tokens / logical token storage | 131,072 / 4 MiB |
| Sections / fields per section / entries per layer | 256 / 512 / 8,192 |
| Individual identifier / decoded text | 128 bytes / 64 KiB |
| String items per list / across layer | 4,096 / 8,192 |
| Aggregate identifier / value payload | 1 MiB / 4 MiB |

All limits must be nonzero. Lexer capacity is bounded by
`min(max_lex_tokens, max_lex_token_storage_bytes / size_of::<Token>())`, including any EOF token.
The iterator is consumed incrementally; capacity is reserved only after checking the next extent.
No `Lexer::into_vec` or full-ceiling allocation is used. The token ceilings are explicit format policy,
independent of raw byte limits. They bound requested logical storage, not allocator residency.

Decoded strings use the actual donor twice: an allocation-free byte count, builder charge, exact
capacity reservation/fill and exact-length check. A list item is charged before decoding/pushing.
Sections, entry paths and field counts are admitted before map growth/name clones. Profile and
declared/entry identifiers, text/list UTF-8, boolean1 and integer8 bytes retain shared budget rules.
Decimal normalization uses a fixed stack sink followed by native i64 conversion.

One shared first-failure latch covers donor errors, decoder refusal and receiver errors. `clear`
cannot reset a rejected sink. Later callbacks cannot grow or complete product state after rejection;
finish checks failure before returning a document. An aggregate callback returning false also records
failure; refusal alone is never success authority. Failures return closed content-free errors without
partial documents, donor diagnostics, raw values or private paths.

## Donor and identity

The sole direct grammar edge is `toml_parser = "=1.1.4"`, defaults off, feature `std` (`alloc/std`).
Cargo ignores requirement build metadata; the lock/checksum bind `1.1.4+spec-1.1.0`. The existing
resolved transitive edge is `winnow 1.0.4`, with no enabled features or further normal dependencies.
No direct high-level `toml`/Serde edge or Serde downgrade is introduced. Parser `unsafe`, debug and
SIMD features stay disabled. The high-level Deserializer was excluded because it materializes a
document tree before the owner visitor.

Verified artifact identities:

- Parser SHA256 `bed810ee007d8e7f9cf20b4774e7198142fa27186e0f805871da7f3c39eaf333`,
  36,175 bytes, VCS `f72038c2508112dfaaedb227a511f91bf9b2015c`, MIT OR Apache-2.0, MSRV1.85.
- Winnow SHA256 `23b97319f7b8343df12cc98938e5c3eb436064524c8d2b4e30a1d3a36eecdf81`,
  188,513 bytes, VCS `7539ec0fc27144bfdcf9a68b0dbbec48bd0d5bae`, MIT, MSRV1.65.

15/15 parser and 63/63 winnow cached source files matched their verified archives; manifests declare
no build/native target. Exact-version OSV queries on 2026-10-10 at21:07:38Z returned HTTP200 with no
matching advisory. Those observations establish provenance/closure and a bounded advisory screen;
they do not qualify security, corpus compatibility, performance or resource behavior.

Canonical v2 row layouts, domains, algorithm and fingerprint revision remain unchanged. Schema2
changes the snapshot header value `[config_schema_version, selected_profile]` and its resulting
digest. Legacy SHA-v1 restoration is decode-only and cannot construct a current snapshot. Daemon
status preserves fingerprint tags, rejects report/snapshot mismatch and reports the actual snapshot
schema. `supported_file_syntax_profile` describes supported grammar, including for defaults-only
snapshots; it does not assert that a snapshot was parsed from a file.

Fixtures are `AUTHORED_NOT_RUN` under #352. Integrated/native/installed/scale/release qualification
remains #215; runtime application/readback remains #333. Historical configuration1.0 documentation
is retained as a frozen format reference, not current parser authority.
