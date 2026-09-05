# DIRECT read-side preparation

The primary `eliot-searchd` composes this path for `DirectStore::search`:

```text
existing immutable revision identity + length + SHA-256 readback
  -> search-materializer::materialize_utf8
  -> search-unitizer::unitize_text
  -> search-exact::literal::scan_chunks
  -> source-bound byte ranges and existing result handles
```

The same store is used by primary one-shot, persistent and authenticated-proxy search commands.
The old `development::scan_text` remains for explicit one-shot development scans and the legacy
plaintext implementation; the primary revision-protected search path no longer calls it.
No Python process, new service, external crate version or second persistent index is introduced.

## Boundaries

This is read-side algorithm integration, not completion of the canonical durable preparation pipeline.
Preparation is recomputed in memory from verified retained bytes on each search. Its range-only values
are not `UnitManifest`, durable receipts, qualified profiles or source-admission evidence.

The existing SHA-256 source/revision bindings and on-disk format are preserved. They are not cast into
`Blake3Digest32`. The new pure byte/layout entrypoints do not require invented revision numbers or
receipt references. The existing receipt-bound materializer/unitizer APIs remain available and share
the same transformation algorithms; their callers still own exact digest and receipt validation.
The exact-proof API is re-exported unchanged from `search-exact::proof`; the public literal module
supplies matching mechanics only and cannot create an authoritative denominator or complete-negative
proof by itself. The `proof` module is private; existing public imports stay at the crate root.

Durable representation/unit manifests, profile identities and receipts, canonical revision-store
adapter composition, migration of primary control state to redb, live Qdrant transport/publication and
Windows crash, handle-race and encryption-before-publication qualification remain unfinished. No wave or acceptance
gate is advanced. Enabling dependencies does not qualify their capabilities.

## Behavior

UTF-8 bytes are not normalized. Line mapping distinguishes LF, CRLF and standalone CR; byte offsets,
zero-based line numbers and byte columns refer to the exact source representation. This corrects the
old development scanner's LF-only line counting for files containing standalone CR.

Unitization validates the actual line inventory: hidden line terminators, invented unterminated middle
lines and a CRLF falsely presented as two lines are rejected. Whole-line boundaries are preferred;
long lines split only at UTF-8 boundaries. Ordered ranges cover the text without gaps or overlaps.
Boundary lookup uses binary search instead of repeatedly rescanning the complete line inventory.

Literal matching retains KMP state across unit boundaries and finds overlapping matches. A query can
span several units. ASCII-insensitive mode folds ASCII only, never Unicode. No unit text is copied by
the range-only preparation path. Input/chunk ceilings are validated before early result truncation.
Exactly reaching the match ceiling is not itself truncation: an extra actual match establishes that
results were omitted. Corpus-level coverage remains the store's responsibility.

The primary composition uses finite ceilings: 64 MiB source bytes, 64 KiB query bytes, one million
lines/units and 100,000 matches; preferred/hard unit sizes are 16/64 KiB. These are existing DIRECT
byte/result ceilings plus explicit preparation limits, not a published production profile.
NUL/binary content, invalid UTF-8, malformed line maps and exhausted preparation limits cannot become
successful empty results. Per-source failures become source gaps with `complete=false`; unaffected
sources can still be searched. Invalid queries are rejected even when the corpus is empty.

## Verification

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test -p search-materializer -p search-unitizer -p search-exact --locked
cargo +1.98.0 test -p eliot-searchd --bin eliot-searchd --locked
cargo +1.98.0 test -p eliot-searchd --test direct_preparation_process --locked
```

Tests cover shared/bound preparation equivalence, invalid line maps, Unicode boundaries, cross-unit and
overlapping literals, finite limits, query validation on empty input and deterministic comparison with
a simple reference matcher. Five process tests invoke the actual primary binary for cross-unit search,
CR/LF/Unicode coordinates, binary gaps, restart/reindex/historical readback and overlapping matches.

The new Rust code and tests have not been compiled or executed in the authoring environment, which
has no Rust toolchain and cannot resolve installation hosts. Formatting and Clippy are unverified too.
Manual-only CI policy is unchanged. These statements do not claim build, security or product acceptance.
