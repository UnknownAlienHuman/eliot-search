# search-code-enricher — source and removal map

## Current API and callers

| Current symbol/area | Actual behavior / caller reality |
|---|---|
| `parse_rust_no_execute` | Prefix/brace line scanner; not a Rust grammar. |
| `extract_configuration_predicate` | Parses a small local cfg subset; does not evaluate target/build configuration. |
| `extract_structural_facts` | Builds hand-written fact kinds and assigns units by node ordinal/clamping. |
| `extract_structural_relations` | Emits only a subset of the public relation vocabulary. |
| `validate_fact_anchor` | Compares supplied revision/range fields; does not reopen the retained object or recompute the supplied fact digest. |
| `build_enrichment_manifest` / `enrich_code` | Builds tree/facts/relations/receipt structures. External production caller was not found. |
| parser profile / qualification | Validates caller metadata; loads no actual donor parser. |

Production source is one 63,638-byte `lib.rs`; the qualification integration test is another 38,737 bytes.
Do not refactor the file into many modules while preserving the disconnected system.

## Baseline action

Remove the package if no connected outline/navigation consumer is selected. Existing project orientation uses
source paths, prepared headings/sections/code blocks and exact ranges. Text retrieval remains available for
all languages.

Delete:

- line/brace Rust parser;
- profile qualification/receipt model;
- generic fact/relation/assurance manifest;
- ordinal/clamped UnitSet mapping;
- unused relation kinds and fact digests;
- qualification fixtures/status/coverage/package-map entries;
- open donor-program dependencies which have no product consumer.

## Optional syntax outline

If selected after base search, implement one direct owner beside preparation/project orientation:

```text
extract_outline(
  retained representation,
  selected extractor profile,
  RequestContext / preparation context
) -> Vec<OutlineItem> + gaps
```

`OutlineItem` contains source/revision/representation, exact byte range, name, kind, bounded signature and
optional parent range. It does not carry a generic assurance receipt, fact-generation ID, canonical batch
digest or UnitSet ordinal.

### ast-grep option

Use an exact packaged `ast-grep outline` artifact for bounded explicit inputs. Invoke one process for a finite
batch or stdin blob, with no shell/default directory crawl/user rules/network. Bound every input before launch
and bound streamed JSON output. Match every result to the exact input and verify its range. This command may
skip read errors/empty files and uses an unbounded internal channel, so omissions/failures remain gaps.

### Tree-sitter option

Use one exact runtime+grammar+query profile in-process only when packaging/process constraints justify it.
Compile the fixed ELIOT-owned query once, parse exact retained bytes and map exact byte ranges. Do not build a
language framework or accept repository-provided grammars/queries.

Choose one syntax owner. Do not keep both in production.

## Optional semantic import

A concrete producer may later supply bounded SCIP/LSP locations for one selected source view. The importer:

1. validates exact producer/source-view identity;
2. maps document paths only through the frozen view;
3. converts producer positions to exact retained byte ranges;
4. emits the few location records consumed by a real navigation handler;
5. reports missing/incomplete/unsupported relationships explicitly.

Do not create a provider-neutral graph/fact database. Rust-analyzer SCIP document text may be empty and its
relationship fields may be unpopulated; producer presence does not establish capability.

## Deferred profiles

Universal Ctags, SARIF, CodeQL, Semgrep and other analyzers remain separate optional integrations. They are
not dependencies of syntax outline, baseline search or orientation. Reopen a narrow issue only with one
selected producer, corpus, expected output and user-facing consumer.

## Verification if selected

- exact retained source/range mapping;
- malformed/incomplete input yields gaps without false completeness;
- no repository code/network/config execution;
- finite process/input/output/deadline/cancellation bounds;
- unchanged revision/extractor settings reuse the prepared outline;
- changed source/profile regenerates it;
- actual project-guide/navigation consumer uses the result;
- no old heuristic parser or generic fact platform remains reachable.
