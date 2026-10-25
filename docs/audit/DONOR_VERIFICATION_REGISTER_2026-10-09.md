# Donor register — installed state, implementation-time candidates and verified limits

This is the live donor index. It records exact installed versions, uninstalled candidates and upstream behavior
which constrains a concrete ELIOT owner. It does not approve a dependency, reserve a future pin, create a runtime
credential or replace the consuming owner's recipe.

**Current repository checkpoint:** R047 over inspected head
`1eeb41912893d323a677ae5b94c8d45f0a636453`, 2026-10-25.  
**Previous detailed research:**
[`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

A donor has four distinct states:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected to a current caller;
4. behavior measured through the real product path.

Do not collapse these into an “accepted donor” Boolean, receipt or package-status field. Recheck exact current
version, MSRV, advisories, features, transitive closure and native artifact when the real consumer is implemented.

<a id="installed-selected-and-optional"></a>

## Installed dependencies and service baseline

| Donor | Current installed state | Consuming boundary and important limit |
|---|---|---|
| `qdrant-client` / Qdrant | Client locked `1.19.0`; selected server baseline `1.19.0` | One private bridge for publication/query. Normal daemon path remains unconnected. R047 verifies update-mode semantics below; live races remain unrun. |
| `redb` | Locked `2.6.3` | Compact current/published rows and real read snapshots. Transactions do not include Qdrant/files; long readers retain pages. |
| Tokio | Locked `1.53.1` in current async consumers | Process/I/O/deadline primitives. Dropping a future does not cancel arbitrary blocking work or settle an external mutation. |
| `pulldown-cmark` | R031 pins `0.13.5`, checksum `278e9268c93b9e498a2da46c34d90ba93a3255eb99009646398f34bc73ab36eb`, defaults disabled | Materializer uses empty options and `OffsetIter`; version/checksum are transform identity. Connected parser/materialize/replay cases remain #215/#331 evidence. |
| `quick-xml` | Locked `0.42.0` | Existing bounded JATS/XML parser/coordinate consumer, not a generic document-worker platform. |
| `toml_parser` | Locked `1.1.4+spec-1.1.0` | Current low-level legacy decoder, not the selected typed TOML replacement. |
| `aho-corasick` | Locked `1.1.5` | Useful overlapping literal primitive; current helper is not the connected retained-source Exact owner. |
| `unicode-ident` / `unicode-normalization` | `1.0.26` / `0.1.25` | Narrow lexical inputs; normalization does not establish compiler symbol identity. |
| `globset` | Locked `0.4.20` | Admission matching only; not a source inventory owner. |

R031 Actions run 40084893130 passed Rust 1.98 locked check and minimal Clippy for
`search-materializer` plus `search-unitizer`. This is dependency/compiler evidence for those owners only.

## Implementation-time candidates not installed

| Candidate | Intended use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Incremental typed settings and bounded framed JSON | Absent from lock. Foundation adds only root/plain consumers; later fields/dependencies move with their real owner. Preserve layer absence, close nested structs and redact diagnostics. |
| `ignore::WalkBuilder` | Ignore-aware bounded source inventory | Absent from lock. Select exact current version with #271 after the one-file skeleton. A successful filtered walk is not automatically a complete physical inventory. |
| `notify::RecommendedWatcher` | Optional responsiveness after reconciliation works | Absent from lock. Do not implement before #271 demonstrates convergence without notifications. |
| `regex-automata` | Optional bounded Exact regex predicate | Absent from lock. Literal Exact is baseline; select only with a real #248 consumer. |
| PDFium via `pdfium-render` | First optional retained-PDF native-text profile | Absent from lock/runtime. #227 must select one current crate plus matching packaged native PDFium artifact and record API/MSRV/advisory/license/hash/platform evidence. |
| `rmcp` | Optional MCP tools leaf after standalone Search | Absent from lock. Latest stable SDK inspected in R039 was `3.7.0` (2026-10-22, commit `5d6a11f2ae022f513aec0f6e0f10f89a57891a7f`, MSRV Rust 1.88); recheck only with #219. |
| tree-sitter / SCIP producers | Optional source-backed outlines/semantic facts | Add only with a concrete snapshot-bound orientation consumer. |
| concrete model/tokenizer/runtime | Optional embedding/reranking after lexical baseline | No model donor selected. Require measured benefit and one direct consumer/profile. |

These are alternatives or optional additions, not a mandate to install every donor mentioned in research.

<a id="qdrant-update-mode-and-generation-cleanup"></a>

## Qdrant update modes and cleanup boundary

Official Qdrant point documentation states:

- default upsert inserts or overwrites a point;
- `insert_only` inserts only when the ID is absent;
- `update_only` updates only when the ID exists;
- these update modes are available from server 1.17, so the selected 1.19 baseline includes them.

Primary source: [Qdrant points / update mode](https://qdrant.tech/documentation/concepts/points/#update-mode).

ELIOT therefore uses:

```text
new immutable point → explicit insert_only
fixed valid_until closure → explicit non-creating operation
```

Default upsert is forbidden for retry. Exact identical readback accepts a lost-response insert; conflicting state
fails. Individual points are not physically deleted, so a delayed insert cannot recreate a removed ID inside a
live generation.

Rebuild/compaction switches to a never-reused collection. Whole-generation cleanup requires no current/pending
publisher target and no live view/window guard, then terminal collection deletion plus exact exists/get absence.
Never reuse the collection name. If result/absence is uncertain, retain cleanup state and backpressure. Logical
absence is not proof of physical disk erasure, especially until the pinned platform/server case is executed.

The delayed-point/delete race, collection deletion durability and Windows storage behavior remain runtime
verification, not conclusions from API shape.

<a id="native-bm25-and-separate-statistics-scope"></a>

## Qdrant native BM25, scope and IDF

Use Qdrant native BM25 rather than a second local numerical engine. For every sparse query leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

`idf.corpus` is independent of the retrieval filter and does not exclude hits. In Qdrant 1.19 it is shard-local.
An empty statistics corpus removes rarity/length signals rather than falling back to global IDF. Missing or
incorrect statistics state is not known-empty Search.

Pinned 1.19 source shows filtered IDF collecting the selected population before walking queried postings; top-k
does not bound upstream work. Preserve semantics and measure connected cold/warm broad/small scopes before adding
caching or another layout.

Primary references:

- [native BM25](https://qdrant.tech/documentation/inference/inference-bm25/)
- [per-tenant/scoped statistics](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
- [Qdrant 1.19.0 IDF source](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
- [bridge owner](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#scope-and-idf)

## Qdrant word tokenization is not exact identifier identity

The 1.19 word tokenizer splits on non-alphanumeric characters. Punctuated identifiers may collapse to the same
word sequence. Exact literal/name/location operations remain independent of BM25 candidates.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>

## Markdown OffsetIter correction

At R028, 0.13.1 used `Parser::new_ext(source, Options::empty()).into_offset_iter()` and a printable blank-
remainder input could reach the donor. `pulldown-cmark 0.13.5` shipped 2026-10-21 with the maintained correction.
R031 updated Cargo pin, lock and materializer identity together. Old prepared output is not relabelled as 0.13.5.
#441 is closed as the dependency task; actual materialize/replay evidence remains #215/#331.

<a id="typed-configuration-donor-boundaries"></a>

## Typed configuration donor limits

The current lock does not contain high-level `toml`, Serde or `serde_json`. The
[config owner](../../crates/search-config/FUNCTIONS.md#layer-semantics) defines incremental fields, precedence,
whole-list replacement, path bases and validation.

For the foundation consumer:

- preserve absence in sparse layer structs;
- close each nested struct; do not casually combine `deny_unknown_fields` with `flatten`;
- TOML is UTF-8, while native OS paths supplied outside the file remain OS values;
- map syntax errors to bounded redacted diagnostics;
- distinguish malformed layer from invalid effective settings;
- refuse invalid settings before root effects;
- add no future source/Qdrant/query fields before their consumer exists.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>

## Exact matcher seam

`search-exact::literal::scan_chunks` carries Aho state across supplied UTF-8 chunks and preserves overlapping
matches, but builds the NFA per invocation, requires pre-supplied chunks and lacks cancellation inside the byte
loop. Compile once per Exact request, reset at source boundaries and feed bounded blocks from the retained reader.
Raw bytes, decoded text and derived representations remain distinct predicates.

## Currentness rule

Package caller searches and source-size measurements are revision-specific evidence. Before deletion or donor
adoption, recheck the exact current branch across manifests, Rust/non-Rust callers, features, tests and workflows.
Record inspected SHA, selected consumer, exact version/artifact and unrun work in the registry.
