# search-subject-resolver — real locations and material ambiguity

Resolve a user's selected entity to useful source locations and show ambiguity. Follow the
[architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [query cutover](../README.md). A fixed recipe ladder
is not the product.

## Revision-specific evidence

At the established audit revision, `resolve_subject` was called by package unit tests. Callers supplied candidate
steps and completeness; the package ordered them but read no cursor, index, manifest or retained bytes. Receipt
and revalidation helpers maintained supplied context/fence metadata rather than producing missing evidence. This
is historical caller evidence, not permanent current-branch truth.

Before deletion, merging or connection, search current manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, features, targets, tests and workflows. Name every surviving consumer and replace only its real
behavior in the same cut.

## Minimal connected behavior

1. Resolve an explicit source/revision/range through the pinned manifest and retained-byte lookup. A missing,
   stale or ambiguous explicit location stays visible and is never silently replaced.
2. For a cursor, use an actual containing prepared/syntax definition only when exact mapped ranges exist.
3. When a selected semantic producer exists, use its occurrences in the same published snapshot. A local symbol
   also needs document identity; names are not global identity across repositories/configurations.
4. Otherwise use bounded lexical/name retrieval to nominate possible definitions and preserve the match basis,
   qualifiers/signatures, truncation and ambiguity.

A mention is not necessarily a definition. Same names, supplied equivalence receipts or rename hypotheses do not
establish identity. A truncated candidate set cannot prove uniqueness; missing coverage cannot prove absence.

Keep useful selection/order/ambiguity logic in query composition or a small helper. Remove mandatory ladder
materialization, equivalence/receipt/revalidation machinery and duplicate candidate copies with their last
callers. Reuse ordinary evidence validation and the existing `QueryWindow`.

## Completion

Explicit locations resolve exactly or fail truthfully; same-named definitions remain ambiguous unless real
context disambiguates; lexical nomination does not become semantic identity. If no substantial independent
algorithm remains, inline the helpers and delete the package. After code, run minimal Rust 1.98 check/Clippy for
actual query/preparation/source/protocol consumers.
