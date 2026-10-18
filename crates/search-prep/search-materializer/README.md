# search-materializer

Convert retained source bytes into text and useful source coordinates. A transformation that changes
bytes must describe its mapping and loss; search results must not present transformed offsets as exact
original offsets.

Read the [implementation guide](../../../docs/implementation/README.md) for integration and
[FUNCTIONS.md](FUNCTIONS.md) for actual functions. [AGENTS.md](AGENTS.md) covers changes here.

## Current code

| Area | What exists |
|---|---|
| [Text/code pipeline](src/product/pipeline.rs) | Retained revision read, explicit encoding, decoding, newline normalization, coordinate/loss maps and immutable product. |
| [Markdown](src/markdown/pipeline.rs) | Implemented raw-backed observations using the existing pulldown-cmark parser. HTML and link targets remain inert. |
| [JATS](src/jats/pipeline.rs) | Implemented strict UTF-8 XML subset with mapped observations using quick-xml. This is not full JATS schema validation. |
| [DIRECT preparation](src/legacy_direct.rs), [store codec](src/legacy_store.rs), [artifact I/O](src/legacy_artifact.rs) | Existing old daemon framing, references, manifests and real immutable filesystem mechanics. |

The new product and Markdown/JATS APIs are implemented. They are not yet connected to the production
daemon ingestion path through a real `RevisionReadPort`. The daemon's existing DIRECT path uses the old
preparation APIs. Optional provider qualification functions currently refuse; that does not block
supported text/code preparation.

## Simplify while connecting

Use one materialization result through unitization. The current Markdown/JATS verifiers reopen and
rebuild the entire source product, and the structural unitizers call them again. Keep validation of
external or persisted inputs at their acceptance boundary; a private immutable product passed directly
from its producer does not need another source read, parser run and replay receipt.

The exact UTF-8 path currently copies text in both
[decode](src/decode/engine.rs) and [preserve normalization](src/normalize/engine.rs).
Transfer the owned buffer and reuse unchanged text. UTF-16 decoding should collect line boundaries
as it decodes, without two temporary arrays containing every scalar.

Consolidate [artifact filesystem mechanics](src/legacy_artifact/context.rs) with the existing revision
byte writer/reader. Materializer owns preparation contents and maps; that does not require its own
parallel staging/publication engine. The current old-format names, receipts, cleanup disposition and
hash domains are inventory, not a compatibility obligation.

The current UnitSet ingress accepts exact byte-identical UTF-8. Broader decoding in this package does
not make all transformed output supported by unitization.
