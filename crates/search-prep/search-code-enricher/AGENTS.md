# Working on search-code-enricher

This package is optional and currently disconnected. Read [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md)
and the [implementation guide](../../../docs/implementation/README.md).

Do not split, repair or expand the 63.6 KiB heuristic Rust parser/fact/receipt system. Do not add Tree-sitter,
SCIP, Ctags, SARIF, CodeQL or Semgrep merely because donor issues exist. First identify one concrete
source-backed outline/navigation consumer. If none is selected, delete the package, tests and dependencies.

A selected syntax outline uses exactly one parser owner and emits only bounded name/kind/signature/exact-range
records over retained bytes. It owns no source admission, UnitSet identity, Qdrant schema, access, graph,
receipt or currentness. Missing extraction remains a gap; text search continues.

Semantic locations or analyzer findings are separate optional profiles with their own real producer and
consumer. They cannot block baseline Search or orientation.

If implementing one selected profile, change its actual preparation/query consumer in the same slice and
remove the old heuristic parser/manifest/receipt code. Run minimal Rust 1.98 compiler/Clippy for those actual
consumers. Qualification self-tests of a disconnected package are not delivery.
