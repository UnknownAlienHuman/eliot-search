# Working on code navigation data

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [local integration](README.md).
The goal is useful definitions, source locations and related evidence from retained revisions.

Use one syntax extractor. The selected small integration is `ast-grep outline` over bounded batches
of retained files during preparation. Reuse its output for unchanged revisions. Direct Tree-sitter
is an alternative when in-process byte input is needed, not a second mandatory parser. Neither
syntax mechanism resolves types, proves runtime behavior or supplies a complete call graph.

Keep compact derived records: source/revision, actual ranges, name/kind and useful signature or role.
Connect records to existing units by their coordinates; a syntax-node ordinal is not a unit identity.
A definition spanning several units keeps its full source range. Do not fabricate one containing unit.
Use existing preparation/publication and source validation, not a new code-fact database or registry.

Import optional SCIP facts only for the source snapshot actually indexed. Preserve the producing
language/configuration and convert its positions against those bytes. Rust-analyzer's inspected SCIP
writer omits relationships, so an implementation edge cannot be inferred from that schema alone.
Generating a semantic index is explicit preparation; ordinary queries do not start Cargo, build
scripts, procedural macros or language servers.

Replace the custom line parser and its guessed relations together with the concrete consumers.
Simplify profile/receipt APIs that only certify supplied labels. Retain actual source identity,
coordinate checks, resource bounds and missing/degraded data. Unsupported language or missing semantic
data still permits ordinary text retrieval; it does not justify invented symbol links.

Check the behavior being connected: multiline declarations, braces inside strings/comments, duplicate
names, Unicode coordinates, units crossed by a definition and stale external index locations.
Package boundaries and old logical contracts do not require permission issues or a compatibility layer.
