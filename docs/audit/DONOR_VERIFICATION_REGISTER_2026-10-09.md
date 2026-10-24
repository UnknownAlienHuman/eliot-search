# Donor register — installed state, selected use and verified limits

This is the single live donor index. It records exact installed versions, selected-but-absent dependencies and
upstream behavior that materially constrains an ELIOT owner. It does not approve a dependency, create a runtime
credential or replace the consuming owner's implementation recipe.

**Inspected source:** R031 head `da382b66`, reconciled by R032 on 2026-10-24.
**Previous detailed register:** [`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

When a selected version or operation changes, verify official versioned documentation/source, record the exact
pin and applicability here, then update the consuming owner and code together.

<a id="installed-selected-and-optional"></a>

## Installed dependencies and service baseline

| Donor | Current state | Consuming boundary and important limit |
|---|---|---|
| `qdrant-client` / Qdrant | Client locked `1.19.0`; selected server baseline `1.19.0` | One SDK bridge for publication/query. Real adapter exists but normal daemon route is unconnected. Native BM25, scope/IDF and late-write limits are below. |
| `redb` | Locked `2.6.3` | Compact current/published rows and real read snapshots. Transactions do not include Qdrant/files; long readers retain pages. |
| Tokio | Locked `1.53.1` in current async consumers | Reuse process/I/O/deadline primitives. Dropping a future does not cancel arbitrary blocking work or settle an external mutation. |
| `pulldown-cmark` | R031 pins `0.13.5`, checksum `278e9268c93b9e498a2da46c34d90ba93a3255eb99009646398f34bc73ab36eb`, defaults disabled | Existing materializer uses empty options and `OffsetIter`; version/checksum are part of preparation identity. #441 is closed as the dependency/identity cut. Connected parser/materialize/replay cases remain #215/#331 evidence. |
| `quick-xml` | Locked `0.42.0` | Existing inert JATS/XML parsing/coordinate consumer, not a document-worker platform. |
| `toml_parser` | Locked `1.1.4+spec-1.1.0` | Current low-level legacy config decoder. It is not the selected typed TOML replacement. |
| `aho-corasick` | Locked `1.1.5` | Useful overlapping literal primitive. Current helper rebuilds per call and is not an incremental retained-source request owner. |
| `unicode-ident` / `unicode-normalization` | `1.0.26` / `0.1.25` | Narrow code-identifier inputs; do not infer compiler symbol identity from normalized lexical aliases. |
| `globset` | Locked `0.4.20` | Admission matching only; not a source inventory owner. |

R031 Actions run 40084893130 passed Rust 1.98 locked `check` and minimal Clippy for
`search-materializer` plus `search-unitizer`. That is dependency resolution/compiler evidence for those owners,
not parser behavior, daemon, Windows, live Qdrant or product evidence.

## Selected but not installed

| Candidate | Selected use | Adoption rule |
|---|---|---|
| High-level `toml` + Serde; `serde_json` | Typed sparse settings and bounded framed JSON | Pin only with #333/#280 consumers. Close nested structs, preserve layer absence and redact diagnostics. |
| `ignore::WalkBuilder` | Bounded ignore-aware source inventory | Select exact version with #271 implementation. A successful filtered walk is not automatically a complete physical inventory. |
| `regex-automata` | Optional bounded Exact regex predicate | Select only with #248; freeze syntax/Unicode/newline/resource choices and do not claim chunk streaming unless the chosen API preserves it. |
| PDFium via `pdfium-render` | First optional retained-PDF native-text profile | Select/package a matching PDFium artifact with #227. Extracted page text is a derived representation, not binary-PDF byte identity. |
| `rmcp` | Optional MCP tools leaf after standalone Search | Use official SDK only with #219 and the real local client. No second query/session/source authority. |
| tree-sitter / SCIP producers | Optional source-backed outlines/semantic facts | Add only with a concrete snapshot-bound orientation consumer; syntax does not fabricate compiler call truth. |

These are alternatives or optional additions, not a mandate to install every donor mentioned in research.

## Qdrant: scope, IDF and cost

Qdrant native BM25 accepts document/query text. Use it instead of implementing a second numerical BM25 engine.
Publish explicit document/query options; omitted language/stemmer/stopword defaults are not neutral for code.

For each sparse query leaf distinguish:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

The `idf.corpus` filter is independent of the retrieval filter and does not exclude hits by itself. It is
available from Qdrant 1.19.0 and is shard-local. An empty statistics corpus removes rarity/length signals rather
than falling back to global IDF. Missing or incorrect statistics state is not known-empty Search.

Pinned 1.19.0 source shows filtered IDF first collecting the selected population into sorted IDs or a dense
mask, then walking queried postings. Top-k therefore does not bound upstream work; broad scopes can enumerate a
large population before ranking. Payload indexes help filtering but do not make scoped IDF constant-time. Keep
required semantics and measure cold/warm scope sizes through the real path before selecting a cache/layout change.

Primary references:

- [Native BM25](https://qdrant.tech/documentation/inference/inference-bm25/)
- [Per-tenant/scoped statistics](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
- [Qdrant 1.19.0 IDF source](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
- [Bridge owner](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#schema-scope-and-idf)

## Qdrant tokenizer is not exact identifier identity

The 1.19.0 word tokenizer splits on non-alphanumeric characters. `parse_manifest`, `parse-manifest` and
`parse::manifest` collapse to the same word sequence; `C++` and `C#` lose punctuation. Adding the original spelling
to text improves recall but does not preserve an indivisible exact name.

Exact literal/name/location requests remain independent of BM25 candidates. Use retained-source scanning or a
real snapshot-bound syntax/semantic producer. See the [exact owner](../../crates/search-query/search-exact/FUNCTIONS.md)
and [query behavior](../../crates/search-query/README.md).

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>

## Markdown OffsetIter correction

At R028 the installed 0.13.1 adapter used
`Parser::new_ext(source, Options::empty()).into_offset_iter()` and the printable input
`"- [o]:u\n    \t"` could reach the donor under sufficient limits. Upstream issue #1129 described an empty
`TightParagraph`/OffsetIter failure; the original NUL-bearing diagnostic is rejected earlier by ELIOT.

`pulldown-cmark 0.13.5` shipped 2026-10-21 and includes the maintained blank-remainder correction plus other
parser/range changes. R031 updated the exact Cargo pin, lock and materializer donor descriptor together while
preserving empty options and `OffsetIter`. Old prepared output is not relabelled as 0.13.5 output.

[#441](https://github.com/UnknownAlienHuman/eliot-search/issues/441) is closed as this dependency/identity task.
The printable case, bounded list/blockquote/CRLF variants, complete event/range checks and actual
materialize/replay run remain connected-product evidence under #215/#331, not another prerequisite framework.

Primary references:

- [0.13.5 release](https://github.com/pulldown-cmark/pulldown-cmark/releases/tag/v0.13.5)
- [release PR #1181](https://github.com/pulldown-cmark/pulldown-cmark/pull/1181)
- [materializer owner](../../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction)

<a id="typed-configuration-donor-boundaries"></a>

## Typed configuration donor limits

The current lock does not contain high-level `toml`, Serde or `serde_json`. Verified constraints for #333:

- typed TOML decoding requires the selected `parse`/Serde features;
- `deny_unknown_fields` closes each nested struct and is incompatible with `flatten`;
- layer structs must preserve absence rather than applying application defaults at each layer;
- TOML documents are UTF-8 strings, so native OS paths supplied outside the file remain OS values;
- decoder errors may include input context; map them to bounded redacted diagnostics;
- malformed syntax and invalid effective settings are distinct; startup refuses invalid values before owner
  effects.

The [config owner](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation) owns precedence,
whole-list replacement, path bases, validation and concrete examples. No dotted-key database or second settings
registry is selected.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>

## Exact matcher seam

`search-exact::literal::scan_chunks` currently carries Aho state across supplied UTF-8 chunks and preserves
overlapping matches, but builds the NFA per invocation, requires chunks to be pre-supplied and lacks cancellation
inside the byte loop. It is a useful primitive, not the connected request owner.

Compile once per Exact request, reset state at source boundaries and feed bounded blocks from the actual retained
reader with cancellation checks. Raw bytes, decoded text and derived representations remain distinct predicates.
Do not replace overlapping behavior with a convenience iterator or infer complete no-match from an incomplete
inventory.

## Donor maintenance rule

Record four states separately:

1. upstream behavior inspected;
2. ELIOT use selected;
3. dependency/service actually installed and connected to a caller;
4. behavior measured through the real product path.

Do not collapse them into an “accepted donor” boolean, per-file stamp or runtime receipt. Preserve immutable
links to older research when shortening this live register; Git history contains the superseded detail.
