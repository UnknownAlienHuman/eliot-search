# search-code-enricher — optional outline, not a code-intelligence platform

Code orientation should help a user choose the next file/definition to read. It is optional enrichment over
the same retained source, unit and query path; it is not a compiler, language server, graph database or
second index.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [preparation map](../README.md) and
[source map](FUNCTIONS.md).

## Verified current state

`src/lib.rs` is **63,638 bytes** of production Rust. Its only external calls found in public main are the
package's own qualification tests.

The implementation:

- scans Rust text line by line with prefix/brace heuristics;
- assigns facts to units by node ordinal rather than exact interval mapping;
- carries parser-profile qualification and generic assurance receipts;
- advertises relation kinds which its extractor does not produce;
- builds trees, facts, relations and manifests without a production preparation/query consumer.

It does not provide reliable definitions/references/calls and should not be connected or expanded.

## Baseline product

Useful orientation already exists without a code-fact plane:

```text
selected source inventory
+ paths and file types
+ prepared headings / sections / code blocks
+ exact unit ranges
+ entry/config/test/doc naming hints
→ small project guide and next-read locations
```

Base Search must ship on this information first. Unsupported precise code navigation is reported honestly.

## Disposition

Delete the current package, tests and workspace dependencies unless a concrete connected consumer is selected
before implementation. Do not replace it with a generic Tree-sitter/SCIP/Ctags/SARIF framework.

A future selected profile is one narrow producer with one real user-facing query:

### Syntax outline

Use one donor only:

- packaged `ast-grep outline` for bounded explicit retained files; or
- one in-process Tree-sitter grammar/query profile when packaging/process constraints justify it.

Output only compact source-backed records:

```text
OutlineItem {
  source revision / representation
  exact byte range
  name
  kind
  bounded signature
  optional parent range
}
```

No common fact ontology, producer registry, profile receipts, assurance graph or relation database is needed.
A parser error or omitted file remains an explicit gap. The existing text search continues.

### Optional semantic locations

Import SCIP/LSP locations only after a concrete feature needs precise definitions/references and a selected
producer can generate them for the exact retained source view. Keep the importer next to that adapter and
store only the bounded locations the user-facing handler consumes.

Rust-analyzer SCIP does not supply all relationships/implementors, can load Cargo/build/procedural-macro
state and is not ordinary search work. Never run it implicitly for every query or source change.

### Optional static-analysis findings

SARIF/CodeQL/Semgrep is a separate optional product. It does not belong in baseline preparation or in this
package until a real analyzer workflow, corpus and query consumer are selected.

## Rules for any selected profile

- parser/indexer reads only exact retained bytes or one explicitly materialized read-only view;
- path/URI never admits a source;
- every returned range is verified against the exact retained representation;
- source view, producer/version/settings and completeness gaps remain explicit;
- finite input/output/process/deadline/cancellation limits apply;
- no repository code, hook, filter, macro, build or network execution unless a separate selected profile
  explicitly permits it;
- no raw donor types in public protocol/Qdrant payload;
- no second source/access/publication/currentness owner;
- no mandatory profile merely because a binary/parser is installed.

## Completion

This package is optional and cannot block the connected source→Qdrant→excerpt path. If a syntax-outline
feature is selected, close only when one real source-backed outline record reaches the project guide or
navigation response, and the old heuristic parser/receipts/unused relation model are gone. Otherwise delete
the package and close the optional programme `not_planned`.
