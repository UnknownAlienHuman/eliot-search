# Code map — search-materializer

The [implementation guide](../../../docs/implementation/README.md) owns the connected target.
These are delivered functions and concrete simplifications, not a second logical API specification.

## Text/code functions

| Location / symbols | Current behavior |
|---|---|
| [profile/validate.rs](src/profile/validate.rs): `validate_materializer_profile` | Checks supported encoding, normalization, mapping and size choices. [profile/change.rs](src/profile/change.rs) classifies existing profiles. |
| [request/validate.rs](src/request/validate.rs): `validate_materialization_request` | Checks the current source/revision/profile request fields. Change obsolete identity/admission fields with their real producers. |
| [product/read.rs](src/product/read.rs): `open_exact_revision`, `RevisionReadPort` | Reads supplied retained bytes and recomputes their content digest/length against the request. The production daemon adapter is still missing. |
| [decode/detect.rs](src/decode/detect.rs): `detect_or_validate_encoding`; [decode/engine.rs](src/decode/engine.rs): `decode_text_or_code` | Implements explicit UTF-8/UTF-16 decoding and native line coordinates. |
| [normalize/engine.rs](src/normalize/engine.rs): `normalize_representation` | Preserves newlines or normalizes them to LF under the selected profile. |
| [maps/build.rs](src/maps/build.rs), [maps/validate.rs](src/maps/validate.rs) | Builds/checks coordinate and loss maps. Assurance follows their actual loss and ambiguity. |
| [product/pipeline.rs](src/product/pipeline.rs): `materialize_text_or_code` | Runs the above stages and returns `MaterializationProduct` with private fields. It performs no durable publication. |
| [product/verify.rs](src/product/verify.rs): `verify_materialization` | Rehashes product text/maps and checks the source/profile binding. |
| [product/codec.rs](src/product/codec.rs): `canonicalize_materialization` | Encodes existing metadata/maps. This is not persistence of the source or canonical text. |
| [product/verify.rs](src/product/verify.rs): `prepare_admission` | Copies product metadata into a plan after checking a nonzero step value. It performs no I/O and has no production caller. Do not add a publication stage just to consume it. |
| [utf8/materialize.rs](src/utf8/materialize.rs): `materialize_utf8` | Existing simpler owned-buffer UTF-8 preparation used by old DIRECT code. Reuse useful mechanics without preserving the old application. |

## Markdown and JATS

| Location / symbols | Current behavior |
|---|---|
| [markdown/profile.rs](src/markdown/profile.rs): `MarkdownProfile::new` | Selects strict raw UTF-8 and current parser/limit choices. |
| [markdown/pipeline.rs](src/markdown/pipeline.rs): `materialize_markdown` | Calls `materialize_inner`, which calls `materialize_text_or_code`, then collects source spans and selected mapped values. |
| [markdown/pipeline.rs](src/markdown/pipeline.rs): `verify_markdown` | Calls `materialize_inner` again, rereads/reparses the source and compares the whole rebuilt product. |
| [jats/profile.rs](src/jats/profile.rs): `JatsProfile::new` | Selects the implemented strict UTF-8 subset. |
| [jats/pipeline.rs](src/jats/pipeline.rs): `materialize_jats`, `verify_jats` | Both call `build`; it runs the raw producer, re-verifies that product and parses XML. Verification rebuilds the whole product. |

Markdown node/inline spans and JATS element/attribute/text/comment observations are implemented.
Mapped normalized values are not exact source subanchors. Markdown reference observations can be
incomplete; the JATS subset is not a general schema validator.

The structural unitizers currently call these full replay verifiers before building their units.
Change those inputs/callers to consume the single private immutable product. Retain independent
validation when accepting persisted or external data, not between ordinary trusted pure functions.
No additional revision port, replay receipt or second parser is needed for that handoff.

## Specific reductions

- `decode_text_or_code` uses `text.to_string()` for UTF-8; `preserve_exact` copies it again.
  Consume the existing owned buffer and move/reuse unchanged text. In the pinned Rust 1.98.0
  [String::from_utf8 implementation](https://github.com/rust-lang/rust/blob/1.98.0/library/alloc/src/string.rs),
  valid input becomes `String { vec }` without copying the buffer. Allocate new text when a real
  transform requires it.
- `decode_utf16_units` retains `native_of_char`; `split_transcoded_lines` builds another scalar
  vector before scanning line ends. Track native/scalar positions and line endings during decoding.
  Temporary coordinate storage should follow emitted lines, not every scalar.
- Keep the existing [pulldown-cmark 0.13.1 offset iterator](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.1/pulldown-cmark/src/parse.rs):
  `Parser::new_ext(..., Options::empty()).into_offset_iter()` already supplies original ranges.
  [jats/parse.rs](src/jats/parse.rs) already uses quick-xml borrowed `read_event` events.
  These donors are integrated; adding another wrapper does not improve the preparation path.

## Existing DIRECT storage to consolidate

[legacy_store.rs](src/legacy_store.rs) owns binding/reference/manifest codecs;
[legacy_inventory.rs](src/legacy_inventory.rs) owns old filename classification.
[legacy_artifact.rs](src/legacy_artifact.rs) and its
[context kernel](src/legacy_artifact/context.rs) perform real immutable file I/O.

The production [preparation command](../../../bins/eliot-searchd/src/preparation_composition.rs)
calls `prepare_revision_canonical` / `prepare_root` in
[batch.rs](../../../bins/eliot-searchd/src/preparation_store/kernel/batch.rs) for an existing old root.
The daemon's [persist_canonical](../../../bins/eliot-searchd/src/preparation_store/kernel/persist.rs)
uses this artifact writer, whose stage and final readbacks both read the whole object. It then calls
`read_object` for another complete read. For plaintext, retain the actual verified bytes or the caller's
buffer already compared with final bytes, instead of reading it again. Metadata alone is insufficient;
protected decoding has its own actual transform to validate.

The actual query loader also copies plaintext in
[decode_object](../../../bins/eliot-searchd/src/preparation_store/kernel/codec.rs), then copies the body
again in [load](../../../bins/eliot-searchd/src/preparation_store/kernel/load.rs). Move the owned decoded
buffer and borrow its body range. Consolidate file mechanics into the existing byte owner with the
daemon consumers; retain preparation contents here. Old schemas, crypto tags, receipts and deferred
cleanup are not obligations for the new format.
