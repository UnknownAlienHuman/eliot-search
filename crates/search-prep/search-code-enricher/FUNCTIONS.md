# search-code-enricher functions

## Existing functions

These are implemented in [src/lib.rs](src/lib.rs); only package tests currently call `enrich_code`.

| Symbols | Actual behavior |
|---|---|
| `validate_parser_profile`, `compare_profile_change` | Validate supplied profile/qualification metadata and classify profile changes; they do not load a donor parser. |
| `parse_rust_no_execute` | Scan immutable `RustRepresentation` line by line using prefix/brace heuristics, with budget/cancellation and a digest callback. |
| `extract_configuration_predicate`, internal `classify_evidence_role` | Parse the local bounded cfg syntax and classify the hand-built node kinds; no target configuration evaluation or resolved caller analysis. |
| `extract_structural_facts`, `extract_structural_relations` | Build bounded facts and a subset of descriptive relations from those nodes. Current node-ordinal unit assignment must be replaced by range lookup. |
| `validate_fact_anchor` | Check matching revision/representation fields, range bounds and UTF-8 slice validity; it does not reopen source storage or recompute the supplied fact digest. |
| `build_enrichment_manifest`, `enrich_code` | Assemble the existing tree/facts/relations/manifest and receipts. No production source capture, publication or query consumer is connected. |

Change these APIs with their actual preparation/query consumers when replacing the scanner.
Do not preserve its intermediate trees, generic assurance receipts or unused relation kinds to satisfy
old tests. The [README](README.md#connect-useful-data) defines the compact output and ownership.

## Syntax extraction

Selected donor: [ast-grep `outline`](https://ast-grep.github.io/reference/cli/outline), released as an
[alpha in 0.44.0](https://github.com/ast-grep/ast-grep/releases/tag/0.44.0). This is a planned integration.
Its bundled extraction rules return file items/direct members with names, kinds, signatures and byte
ranges. It does not resolve references/types or construct a call graph.

Use one process for a bounded batch of explicit immutable file paths of the selected language:

```text
ast-grep outline --lang rust --items all --view expanded --json=stream --threads 1 -- FILE1 FILE2
```

Pass paths as process arguments, not a shell command or a directory/default `.` scan. Map each input
path back to its retained source/revision; do not reinterpret an arbitrary working-tree path as the
indexed revision. For one retained blob, `--stdin --lang rust` accepts its bytes. Do not start a process
for each file or query. Cache the prepared records by retained revision and extractor settings/version,
and re-extract only changed inputs.

The donor caches compiled rules once per command and streams one JSON file object at a time, but
[its 0.44.0 worker](https://github.com/ast-grep/ast-grep/blob/0.44.0/crates/cli/src/outline/extract.rs#L134-L240)
uses an unbounded channel and skips individual read/walk errors. Its
[file reader](https://github.com/ast-grep/ast-grep/blob/0.44.0/crates/cli/src/utils/mod.rs#L111-L121)
checks size after reading. Bound batch count/bytes and each file before launch; bound output and use
the existing operation deadline/cancellation. Match output paths to the input list and account for
empty inputs locally: this version omits empty files. Missing nonempty inputs, unsupported syntax and
failures remain gaps; exit status alone does not prove complete extraction.
Keep donor invocation settings explicit so repository-local custom extraction does not silently change
the prepared data.

If packaging requires in-process parsing of retained bytes, replace this syntax owner with
[Tree-sitter `Parser::set_language` / `parse`](https://docs.rs/tree-sitter/0.25.10/tree_sitter/struct.Parser.html),
the selected grammar's [tag queries](https://tree-sitter.github.io/tree-sitter/4-code-navigation.html)
and [node byte ranges](https://docs.rs/tree-sitter/0.25.10/tree_sitter/struct.Node.html#method.byte_range).
Reuse the parse for extraction and unit mapping. Do not keep both syntax runtimes or build a grammar,
cfg evaluator, name resolver or compiler around captures. Semantic facts follow the separate optional
[SCIP import](README.md#optional-semantic-locations).
