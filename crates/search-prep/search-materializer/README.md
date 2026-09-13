# search-materializer

**C08 — Materialization.**

**Status:** baseline text/source-code materialization, profile validation, coordinate/loss maps, assurance classification, and the optional-provider qualification seam are implemented. Concrete optional document-provider qualification remains gated and no provider acceptance is implied.

Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.

## Owns

- materializer profile contracts
- raw text/source-code baseline materialization
- coordinate map and loss map production
- assurance ceiling classification
- provider qualification seam for optional documents

## Must not own

- selecting a PDF/Office/OCR provider without ADR
- authority or ranking
- executing macros, archive members or remote resources
- claiming exact coordinates after lossy transforms
- opening source stores directly instead of consuming immutable contract inputs

- **Delivery wave:** W2 baseline / P04; optional P17
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
