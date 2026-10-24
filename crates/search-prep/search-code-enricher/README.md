# search-code-enricher — optional outline or deletion

Code orientation should help a user choose the next file or definition to read. It is optional enrichment over
the same retained-source, preparation and query path; it is not a compiler, language server, graph database or
second index.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [preparation map](../README.md) and
[source map](FUNCTIONS.md).

## Inspected revision evidence

At the established audit revision, `src/lib.rs` measured **63,638 bytes** of production Rust and external calls
were found only in package qualification tests. The inspected implementation:

- scanned Rust text line by line with prefix/brace heuristics;
- assigned facts by node ordinal rather than verified source intervals;
- carried parser-profile qualification and assurance receipts;
- advertised relation kinds its extractor did not produce;
- built trees/facts/relations/manifests without a production preparation/query consumer.

Those measurements and caller findings are historical evidence, not a permanent current-branch census. Before
removal, recheck exact current manifests, Rust imports/constructors/path inclusion, features, targets, tests,
workflows and other non-Rust consumers. Do not connect or harden the old framework merely because it survives.

## Baseline orientation

Useful baseline orientation can start from retained facts already needed by Search:

```text
selected source inventory
+ paths and file types
+ prepared headings / sections / code blocks
+ exact unit ranges
+ entry/config/test/doc hints
→ bounded guide and next-read locations
```

Base Search ships first. Unsupported precise definitions/references/calls remain explicit gaps.

## Disposition

If the current caller recheck still finds no production consumer and no concrete selected outline profile,
delete the package, tests and workspace/dependency wiring. Do not replace it with a generic Tree-sitter/SCIP/
Ctags/SARIF framework.

A selected syntax-outline profile uses one parser owner and emits only bounded source-backed records:

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
Parser failure or omitted input remains a gap; baseline text search continues.

Optional semantic locations may be imported only when a concrete feature needs precise definitions/references
and a selected producer generates them for the exact retained view. Keep importer/vendor types beside that
adapter. Static-analysis findings are a separate optional product, not baseline preparation.

## Rules for a selected profile

- read only exact retained bytes or one explicit read-only materialization;
- verify every returned range against the retained representation;
- keep source view, producer/version/settings and completeness gaps explicit;
- apply finite input/output/process/deadline/cancellation limits;
- execute no repository code, hooks, filters, macros, builds or network access unless a separate profile
  explicitly permits it;
- expose no raw donor types in public protocol/Qdrant payload;
- own no second source/access/publication/currentness system.

## Completion

This optional package cannot block source → Qdrant → excerpt. If an outline capability is selected, close only
when one real record reaches the orientation result and the old heuristic/receipt model is gone. Otherwise delete
the confirmed dormant package and mark the optional profile not planned. Package self-tests alone are not
product evidence.
