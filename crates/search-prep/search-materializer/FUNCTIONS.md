# search-materializer — source and cutover map

## Useful current functions

| Location / symbols | Keep / change |
|---|---|
| `product/read.rs`: `RevisionReadPort`, `open_exact_revision` | Connect to the one retained-object reader and its deadline/cancellation context. |
| `decode/*`, `normalize/*`, `maps/*` | Keep concrete decoding, transformation and coordinate/loss behavior; remove avoidable copies. |
| `product/pipeline.rs`: `materialize_text_or_code` | Keep as the single producer of a private immutable `MaterializationProduct`. |
| `markdown/pipeline.rs`, `jats/pipeline.rs` | Keep parser/structure extraction. Feed their product directly to unitization. |
| `verify_markdown`, `verify_jats` | Currently reopen/rebuild/reparse the entire product. Restrict independent verification to persisted/external load boundaries. |
| `legacy_artifact/context.rs` | Duplicate immutable object publication kernel, about 40 KiB before tests; remove after shared-store cutover. |
| legacy DIRECT store/inventory/codecs | Remove with the old application after actual consumers move. |

## Current repeated work

- Markdown/JATS structural unitization may invoke full replay verification.
- Valid UTF-8 is copied during decode and again by exact-preserve normalization.
- UTF-16 handling retains multiple scalar-sized temporary arrays.
- Legacy artifact persistence writes a stage, performs full readbacks, publishes final bytes and can leave
  the stage behind even after success.
- Daemon plaintext load/persist paths can read or copy the complete body again after that verification.

## Required handoff

```text
MaterializationProduct {
  source revision
  profile
  owned text or bounded binary representation
  source/normalized coordinate maps
  loss/unsupported observations
  document/code structure and parser observations
}
```

The product is passed by ownership/reference to unitization. Unitizer produces source occurrences and
ranges; model/tokenizer packing remains a derived projection. No replay receipt or second source read is
required inside the same operation.

## Prepared artifact persistence

1. Encode only the maps/structure/text required by projection/evidence.
2. Write those bytes through `search-revision-store`'s single current-format object primitive.
3. Reopen/validate at the persistent boundary.
4. Commit the small object reference/profile/current-preparation pointer in redb.
5. On restart, load and validate the persisted artifact once; otherwise rebuild from retained source.

The materializer does not own stage filenames, hardlink/rename policy, GC, encryption-key lifecycle or
operation receipts. The shared writer removes its stage after verified success; crash debris is ordinary GC
input.

## Donor use

- `String::from_utf8` can take ownership of a valid `Vec<u8>` rather than copying it.
- pulldown-cmark's offset iterator supplies raw Markdown ranges.
- quick-xml's borrowed event API avoids a second XML tokenizer.
- structure-first split/merge rules follow the selected Docling/Unstructured mechanisms without importing
  their runtimes.

## Behavior to verify with integration

- one retained read and one parse for ordinary preparation;
- exact and mapped ranges after UTF-8/UTF-16/newline handling;
- no complete artifact after cancellation, truncation or unsupported transform;
- persisted artifact restart readback;
- no duplicate filesystem publication owner or success-stage leak;
- bounded peak input, parser, map and output memory.
