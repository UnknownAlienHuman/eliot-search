# search-materializer

**C08 — Materialization.**

**Status:** baseline text/source-code materialization, profile validation, coordinate/loss maps, assurance classification, and the optional-provider qualification seam are implemented. During T02 migration the package also owns the bounded legacy DIRECT preparation-object/reference filesystem lifecycle, frozen binding/reference/manifest schema, and physical-inventory filename/classification grammar. Concrete optional document-provider qualification remains gated and no provider acceptance is implied.

Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.

## Owns

- materializer profile contracts
- raw text/source-code baseline materialization
- coordinate map and loss map production
- assurance ceiling classification
- provider qualification seam for optional documents
- bounded exact legacy preparation-object/reference reads
- no-clobber immutable preparation publication with native-identity fencing
- exact `ELSPRP02` binding and `ELSPRF01` reference layouts
- legacy manifest framing, fixed algorithm/profile tags and validation
- lookup/object digest preimages, bounds and canonical lower-case locator names
- closed `refs`/`objects` tree and final/temporary basename grammar
- stable physical/current inventory classification tags and relative locators

## Must not own

- selecting a PDF/Office/OCR provider without ADR
- authority or ranking
- executing macros, archive members or remote resources
- claiming exact coordinates after lossy transforms
- opening source stores directly instead of consuming immutable contract inputs
- revision CAS / revision-object storage
- DPAPI, keyring or secret ownership
- source-registry or control-journal mutation
- daemon data-root traversal, metadata or platform identity observation

The compatibility filesystem adapter treats bytes as opaque. It neither proves a
materialization profile nor authorizes a source; it only preserves exact bounded
artifacts under qualified platform observations. The pure legacy-store codec and
inventory grammar own persisted bytes, digest/name derivation and closed physical
classification while concrete hashing, DPAPI, filesystem traversal and catalog
overlay remain injected by daemon composition.

- **Delivery wave:** W2 baseline / P04; optional P17
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
