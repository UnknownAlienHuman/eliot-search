# Preparation

A retained revision is decoded/materialized, split into useful units and projected with the selected
lexical profile. Each result keeps the coordinates needed to reopen its source evidence.

| Existing code | Role and integration |
|---|---|
| [materializer](search-materializer/) | Text/code decoding and Markdown/JATS preparation. Reuse the existing decoder/parsers and return the prepared product once. |
| [unitizer](search-unitizer/) | Unit occurrences, coordinates and manifests. Consume the materializer's immutable result; remove redundant full verification/parsing between these in-process owners. |
| [code enricher](search-code-enricher/) | Selected syntax-derived code facts. Do not present tolerant parse output as compiler-resolved truth or require this extension before basic search works. |

Current UnitSet ingress is stricter than materializer decoding: its raw path requires byte-identical
UTF-8 with `ExactBytes` and an empty loss map. Change a transform and its UnitSet/source-coordinate
consumer together; a broader decoder alone does not make the whole pipeline support that format.

Text and maps are derived files, not a second corpus in redb. Reuse one existing immutable-byte writer/
reader and move buffers or carry byte ranges instead of repeatedly cloning the full artifact.
The [implementation guide](../../docs/implementation/README.md) owns the concrete path. Current child
packages may become modules when their dependency boundary no longer serves the application.
