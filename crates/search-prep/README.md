# Preparation — one retained input, one reusable result

<a id="preparation--one-derived-product-one-structural-unitset"></a>

Preparation produces source-backed text and ranges. It owns no object store, Qdrant effect or query session.
[Delivery order](../../docs/implementation/README.md#delivery-sequence) selects what to implement now.

## Connected baseline

```text
retained ObjectRef → verified owned bytes → one materialization → unit ranges
→ one bounded prepared encoding → shared object writer → guarded current outcome → publication
```

Start Stage 2 with one explicitly selected UTF-8 text/code file and unchanged text. Line-preferred ranges are
sufficient; an AST, structural manifest, tokenizer/model or old/new occurrence matcher is not a prerequisite.
Other formats join when their actual retained-input → preparation → evidence path is connected. Do not claim
support from the presence of a parser alone.

<a id="existing-code-disposition"></a>
<a id="boundaries"></a>
<a id="structure-and-reuse"></a>
## Read by change

| Change | Single rule owner |
|---|---|
| Retained read, transform, reuse, current outcome or retry | [Materializer](search-materializer/FUNCTIONS.md) |
| Unit ranges, representation storage or conditional occurrence matching | [Unitizer](search-unitizer/FUNCTIONS.md) |
| Actual retrieval-text composition | [Lexical](../search-lexical/FUNCTIONS.md#retrieval-text-no-alias-inflation) |
| Bounded upload pages and matching evidence | [Projection](../search-index-qdrant/search-projection-planner/FUNCTIONS.md) |
| Immutable file I/O | [Object store](../search-source/search-revision-store/FUNCTIONS.md) |

<a id="removal-target"></a>
<a id="verification"></a>
## Removal and checks

Move actual consumers, then remove duplicate writers, adjacent parser replay and nested legacy manifests.
Do not first migrate every v3/v4/v5 format or construct optional enrichment. Keep the preparation result small;
packages may become private modules. Use minimal changed-owner/consumer check/Clippy after code and the owner's
later behavior cases. The old implementations and their earlier inspections are not a working-product claim.
