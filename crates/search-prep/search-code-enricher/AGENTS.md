# Working on search-code-enricher

Read [README.md](README.md), [FUNCTIONS.md](FUNCTIONS.md), the
[implementation guide](../../../docs/implementation/README.md) and orientation issue
[#281](https://github.com/UnknownAlienHuman/eliot-search/issues/281).

The established audit found a 63.6 KiB heuristic parser/fact/receipt package with no external production caller
at that inspected revision. Treat that as historical evidence. Before deletion, repeat an exact current search
of manifests, Rust/non-Rust callers, features, targets, tests and workflows.

Do not split, repair or expand the old framework. First identify one concrete source-backed outline/navigation
consumer. If none exists at the current head, delete the package, exclusive tests and dependency wiring. If a
consumer exists, name it and replace only the behavior it actually needs.

A selected syntax outline uses one parser owner and emits bounded name/kind/signature/exact-range records over
retained bytes. It owns no source admission, UnitSet identity, Qdrant schema, access, graph, receipt or
currentness system. Missing extraction remains a gap; text search continues.

Semantic locations or analyzer findings are separate optional profiles with their own real producer and
consumer. They cannot block baseline Search or orientation.

If implementing a selected profile, change its actual preparation/query consumer in the same slice and remove
the old heuristic parser/manifest/receipt code. Run minimal Rust 1.98 compiler/Clippy for actual consumers.
Qualification self-tests of a disconnected package are not delivery.
