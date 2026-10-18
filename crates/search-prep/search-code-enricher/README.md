# search-code-enricher

Code navigation data should help a user choose a file or definition to read. It extends the same
retained source, unit and publication path described in the
[implementation guide](../../../docs/implementation/README.md).

## Current source

[lib.rs](src/lib.rs) implements `enrich_code`, a pure Rust-text pipeline called by this package's
tests. There is no production preparation/query caller or parser donor dependency here.
`parse_rust_no_execute` scans lines and counts braces; it is not a Rust grammar. Declaration ranges
cover lines, function classification uses brace depth, and `extract_structural_facts` assigns unit IDs
by node ordinal. The relation enum advertises more kinds than `extract_structural_relations` produces.
These kernels do not establish working definitions/references navigation. [FUNCTIONS.md](FUNCTIONS.md)
lists the real APIs and replacement seam.

## Connect useful data

Start a project map with the selected source inventory, relative paths, prepared headings and unit
ranges. Entry/config/test/doc names are navigation hints. A whole-project graph and semantic analysis
are not prerequisites for a useful map.

For code outlines, use one selected syntax owner: the released
[ast-grep 0.44.0 `outline` alpha](https://ast-grep.github.io/guide/outline-code).
Run bounded file batches during preparation and retain compact name/kind/signature/range records.
Reuse unchanged revision/extractor results; query and continuation read those records. The concrete
batch API and donor limitations are in [FUNCTIONS.md](FUNCTIONS.md#syntax-extraction).

Map every returned range to the actual retained representation and its unit coordinates. Preserve the
definition's full range even if it spans multiple units. Publish useful symbol/name/role fields with
the existing searchable units; retain only additional locations needed for direct navigation.
The projection planner's [PlannedUnit](../../search-index-qdrant/search-projection-planner/src/model.rs)
already accepts optional `entity_kind` and `normalized_symbol_key`; it does not produce those facts.
One unit may contain many symbols, so do not truncate its occurrences to fit that single optional key.
Text retrieval continues when syntax data is unavailable.

## Optional semantic locations

Import a selected indexer's [SCIP documents and occurrences](https://github.com/scip-code/scip/blob/fcb3cb3c679fe2da18141550f864b9e67afab297/scip.proto#L76-L115)
to associate symbol keys with definitions/references. Use the existing source and publication owners;
no general graph engine is needed for bounded symbol-to-location lookup. Keep repository/snapshot
context, and include document identity for document-local symbols.

For Rust, [`rust-analyzer scip`](https://github.com/rust-lang/rust-analyzer/blob/91a3d3b547342c525a1a7f4d0983e6c4f1872d3a/crates/rust-analyzer/src/cli/flags.rs#L183-L197)
is an existing producer, not integrated here. Its inspected writer emits
[empty document text](https://github.com/rust-lang/rust-analyzer/blob/91a3d3b547342c525a1a7f4d0983e6c4f1872d3a/crates/rust-analyzer/src/cli/scip.rs#L217-L226)
and [no symbol relationships](https://github.com/rust-lang/rust-analyzer/blob/91a3d3b547342c525a1a7f4d0983e6c4f1872d3a/crates/rust-analyzer/src/cli/scip.rs#L308-L335).
Bind the artifact to the snapshot used to generate it, convert its position encoding and reopen that
revision before returning evidence. A current pathname alone cannot establish this binding.

SCIP's `is_implementation` field does not mean this producer fills it. Resolved implementors/callers
require an actual provider result, such as a selected LSP server's supported navigation request.
Syntax/name matches remain labelled as such. Rust-analyzer results cover its Rust workspace and
selected configuration, not arbitrary languages or every build variant. Its index generation loads
[Cargo/build output and a procedural-macro server](https://github.com/rust-lang/rust-analyzer/blob/91a3d3b547342c525a1a7f4d0983e6c4f1872d3a/crates/rust-analyzer/src/cli/scip.rs#L51-L64);
ordinary search must not launch that work.
