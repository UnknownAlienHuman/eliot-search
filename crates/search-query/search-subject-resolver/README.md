# search-subject-resolver

Resolve a user's selected entity to useful source locations and show material ambiguity.
Follow the [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md); a fixed recipe ladder is not the product.

## Current source

[lib.rs](src/lib.rs) implements `resolve_subject(request, context, steps, limits)`. Its callers in this
repository are unit tests. The caller supplies `ResolutionStep` candidates and completeness; the
function orders them and returns resolved/ambiguous/incomplete/not-found/scope-empty outcomes. It does not read a
cursor, query an index or discover definitions/references.

`collapse_equivalent_occurrences` groups supplied hypothesis digests when an equivalence receipt is
present. That presence does not establish symbol identity, aliasing or rename equivalence from source.
`issue_resolution_receipt` and `revalidate_resolution` maintain the old context/fence metadata; they
are not missing-data producers.

## Minimal connected behavior

1. Resolve an explicit source/revision/range through the existing manifest and retained-byte lookup.
   For a cursor, select a containing prepared definition when actual ranges identify one; expose
   ambiguity when they do not. There is no reason to execute every inferred resolution stage first.
2. When a semantic index is available, use its symbol/occurrence lookup in the selected published
   snapshot. A SCIP local symbol also needs document identity. A bare qualified name is not a global
   identity across repositories, packages and configurations.
3. Otherwise use existing lexical retrieval to nominate definitions by name. Query published symbol
   fields when a real producer has populated them. Return the actual
   locations, displayed qualifiers/signatures and the match basis. Same-name or structurally similar
   results remain candidates; a rename hypothesis does not become local entity identity.

The optional producer is described in
[code enrichment](../../search-prep/search-code-enricher/README.md). For an already selected LSP
session, [`workspace/symbol`](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/workspace/symbol.md)
can nominate locations; resolve a returned URI-only workspace symbol before claiming an exact range.
Live editor locations must be tied to the corresponding buffer/source revision and position encoding.
No language server is a prerequisite for ordinary retrieval.

Keep this selection/ambiguity logic in the existing query owner or a small helper. Connect real
candidate producers and source validation together; remove unused receipt/revalidation machinery and
duplicate candidate copies with their last consumers. Missing coverage is not a complete absence claim,
and a truncated name search cannot certify that its sole visible match is unique.
