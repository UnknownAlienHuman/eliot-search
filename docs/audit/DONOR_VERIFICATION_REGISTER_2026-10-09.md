# Donor register — installed state, selected candidates and verified limits

This is the live donor index. It records exact installed versions, selected-but-absent candidates and upstream
behavior which constrains an ELIOT owner. It does not approve a dependency, create a runtime credential or
replace the consuming owner's implementation recipe.

**Current repository checkpoint:** R039 over `d3d81ca0fc47a68c03235f21b276af0f46fb64f3`, 2026-10-24.  
**Previous detailed research:** [`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

A donor moves through four distinct states:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected to a current caller;
4. behavior measured through the real product path.

Do not collapse these into an “accepted donor” Boolean, receipt or package-status field.

<a id="installed-selected-and-optional"></a>

## Installed dependencies and service baseline

| Donor | Current installed state | Consuming boundary and limit |
|---|---|---|
| `qdrant-client` / Qdrant | Client locked `1.19.0`; selected server baseline `1.19.0` | One SDK bridge for publication/query. Real adapter exists but the normal daemon path is unconnected. Native BM25, scoped IDF and late-write limits are below. |
| `redb` | Locked `2.6.3` | Compact current/published rows and real read snapshots. Transactions do not include Qdrant/files; long readers retain pages. |
| Tokio | Locked `1.53.1` in current async consumers | Reuse process/I/O/deadline primitives. Dropping a future does not cancel arbitrary blocking work or settle an external mutation. |
| `pulldown-cmark` | R031 pins `0.13.5`, checksum `278e9268c93b9e498a2da46c34d90ba93a3255eb99009646398f34bc73ab36eb`, defaults disabled | Materializer uses empty options and `OffsetIter`; version/checksum are transform identity. #441 is closed as the dependency cut. Connected parser/materialize/replay cases remain #215/#331 evidence. |
| `quick-xml` | Locked `0.42.0` | Existing bounded JATS/XML parser/coordinate consumer, not a generic document-worker platform. |
| `toml_parser` | Locked `1.1.4+spec-1.1.0` | Current low-level legacy config decoder, not the selected typed TOML replacement. |
| `aho-corasick` | Locked `1.1.5` | Useful overlapping literal primitive. Current helper rebuilds per call and is not a connected retained-source Exact owner. |
| `unicode-ident` / `unicode-normalization` | `1.0.26` / `0.1.25` | Narrow code-identifier inputs; normalization does not establish compiler symbol identity. |
| `globset` | Locked `0.4.20` | Admission matching only; not a source inventory owner. |

R031 Actions run 40084893130 passed Rust 1.98 locked check and minimal Clippy for
`search-materializer` plus `search-unitizer`. This is dependency/compiler evidence for those owners only.

## Selected candidates not installed

| Candidate | Selected use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Typed sparse settings and bounded framed JSON | Pin only with #333/#280 consumers. Preserve layer absence, close nested structs and redact diagnostics. |
| `ignore::WalkBuilder` | Ignore-aware bounded source inventory | Select exact current version with #271. A successful filtered walk is not automatically a complete physical inventory. |
| `notify::RecommendedWatcher` | Optional responsiveness hints after reconciliation works | **Absent from the current lock.** Do not implement before #271 removes `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE` and demonstrates startup/explicit/periodic convergence without notifications. Then recheck exact version, MSRV, features, platform backends and advisories with #239. |
| `regex-automata` | Optional bounded Exact regex predicate | **Absent from the current lock.** Literal Exact is baseline. Select only with a real #248 consumer; recheck exact current API/version/MSRV/advisories/features and freeze syntax/Unicode/newline/resource semantics with code. |
| PDFium via `pdfium-render` | First optional retained-PDF native-text profile | Uninstalled candidate. #227 must pin one exact crate version and matching packaged native PDFium artifact with API/MSRV/advisory/features/license evidence. |
| `rmcp` | Optional MCP tools leaf after standalone Search | **Absent from the current lock.** Latest inspected stable SDK is `3.7.0` (2026-10-22, commit `5d6a11f2…`, MSRV 1.88); the stable MCP spec remains `2026-07-28`. Recheck and pin only with the real #219 consumer. |
| tree-sitter / SCIP producers | Optional source-backed outlines/semantic facts | Add only with a concrete snapshot-bound orientation consumer; syntax does not fabricate compiler call truth. |

These candidates are alternatives or optional additions, not a mandate to install every donor mentioned in
research.

<a id="native-bm25-and-separate-statistics-scope"></a>

## Qdrant native BM25, scope and IDF

Use Qdrant native BM25 rather than a second local numerical BM25 engine. For each sparse query leaf distinguish:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

`idf.corpus` is independent from the retrieval filter and cannot exclude hits by itself. In Qdrant 1.19.0 it is
shard-local. An empty statistics corpus removes rarity/length signals rather than falling back to global IDF.
Missing or incorrect statistics state is not known-empty Search.

Pinned 1.19.0 source shows filtered IDF first collecting the selected population into sorted IDs or a dense
mask and then walking queried postings. Top-k therefore does not bound upstream work. Preserve required scope
semantics and measure cold/warm broad/small scopes through the connected product before introducing caching or a
new layout.

Primary references:

- [native BM25](https://qdrant.tech/documentation/inference/inference-bm25/)
- [per-tenant/scoped statistics](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
- [Qdrant 1.19.0 IDF source](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
- [bridge owner](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#schema-scope-and-idf)

## Qdrant word tokenization is not exact identifier identity

The 1.19.0 word tokenizer splits on non-alphanumeric characters. `parse_manifest`, `parse-manifest` and
`parse::manifest` collapse to the same word sequence; `C++` and `C#` lose punctuation. Exact literal/name/
location operations therefore remain independent of BM25 candidates.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>

## Markdown OffsetIter correction

At R028, the installed 0.13.1 adapter used
`Parser::new_ext(source, Options::empty()).into_offset_iter()` and the printable input
`"- [o]:u\n    \t"` could reach the donor. Upstream `pulldown-cmark 0.13.5` shipped 2026-10-21 with the
maintained blank-remainder correction plus other parser/range changes.

R031 updated Cargo pin, lock and materializer donor identity together while preserving empty options and
`OffsetIter`. Old prepared output is not relabelled as 0.13.5 output. #441 is closed as this dependency task;
actual parser/materialize/replay evidence remains #215/#331 work.

<a id="typed-configuration-donor-boundaries"></a>

## Typed configuration donor limits

The current lock does not contain high-level `toml`, Serde or `serde_json`. For #333:

- preserve absence in sparse layer structs instead of applying application defaults in every layer;
- close each nested struct; `deny_unknown_fields` and `flatten` cannot be combined casually;
- TOML documents are UTF-8 strings, while native OS paths supplied outside the file remain OS values;
- map syntax errors to bounded redacted diagnostics;
- distinguish malformed syntax from invalid effective settings;
- reject invalid effective settings before owner effects.

The [config owner](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation) owns precedence,
whole-list replacement, path bases and application validation.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>

## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches across supplied UTF-8 chunks, but builds its
NFA per invocation, requires pre-supplied chunks and lacks cancellation inside the byte loop. It is a useful
primitive, not the connected request owner.

Compile once per Exact request, reset state at source boundaries and feed bounded blocks from the retained reader
with cancellation checks. Raw bytes, decoded text and derived representations remain distinct predicates.

<a id="mcp-rust-sdk-candidate"></a>

## MCP Rust SDK candidate

The stable MCP specification release remains `2026-07-28`. The latest stable official Rust SDK inspected on
2026-10-24 is `rmcp 3.7.0`, tag `rmcp-v3.7.0`, tag object
`cdf95e06d7cc2a13f9ff97e3e5d6cf1226630ae9`, commit
`5d6a11f2ae022f513aec0f6e0f10f89a57891a7f`, published 2026-10-22, MSRV Rust 1.88, Apache-2.0.

The release changes auth/HTTP behavior only; it does not justify enabling those features for the first stdio
leaf. At that tag the smallest relevant candidate closure is:

```toml
rmcp = { version = "=3.7.0", default-features = false,
         features = ["server", "macros", "transport-io"] }
```

This is not an installed or permanently selected pin. #219 must recheck the then-current stable SDK, API, MSRV,
advisories, features and transitive closure when standalone Search has the real consumer.

Primary references:

- [MCP 2026-07-28](https://github.com/modelcontextprotocol/modelcontextprotocol/releases/tag/2026-07-28)
- [rmcp 3.7.0 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.7.0)
- [workspace/MSRV](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/Cargo.toml)
- [feature manifest](https://github.com/modelcontextprotocol/rust-sdk/blob/5d6a11f2ae022f513aec0f6e0f10f89a57891a7f/crates/rmcp/Cargo.toml)

## Maintenance rule

When a selected dependency/service or consuming operation changes, check the latest stable official release,
versioned API, MSRV/advisories/features and source behavior relied upon. Record installed versus candidate state
here, then update the actual owner and code together. Git history preserves superseded research detail.
