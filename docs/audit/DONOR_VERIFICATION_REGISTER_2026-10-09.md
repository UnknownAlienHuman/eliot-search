# Donor evidence register

Separate upstream inspection, selected design, installed dependency and executed product behavior. A source link
or candidate selection is not runtime qualification. Algorithms live in the owning recipes.

R055 rechecks Windows references on 2026-10-28. Other dependency observations below are carried from the
[R054 register](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
and unchanged lock at inspected ELIOT head `45ddf887`; they are not a new upstream-wide version/advisory audit.

<a id="installed-selected-and-optional"></a>
## Installed versus selected

| Dependency | Installed/selected state | Owner |
|---|---|---|
| qdrant-client | Locked 1.19.0; exact 1.20.x server/client upgrade selected, not implemented | Bridge |
| redb | Locked 2.6.3 | Typed metadata and read snapshots |
| Tokio | Locked 1.53.1 | Actual async I/O/process consumers |
| pulldown-cmark | Locked 0.13.5, defaults disabled | Markdown OffsetIter |
| quick-xml | Locked 0.42.0 | XML/JATS preparation |
| toml_parser | Locked 1.1.4+spec-1.1.0; legacy, not the typed replacement | Config |
| aho-corasick | Locked 1.1.5 | Literal Exact primitive |
| Unicode/glob helpers | unicode-ident 1.0.26; unicode-normalization 0.1.25; globset 0.4.20 | Narrow existing consumers |

High-level toml/Serde/serde_json, ignore, optional notify, PDFium/pdfium-render, rmcp, syntax and model donors are
implementation-time candidates, not installed capabilities. Recheck exact version, features, MSRV, advisories,
license/native artifacts and real API at adoption. Existing transitive crates do not establish a connected feature.

<a id="qdrant-120-direct-bm25"></a>
## Qdrant 1.20 scoring text

The prior inspection selected vectorless scoring full-text BM25 and TextQuery instead of sparse inference.
[Lexical](../../crates/search-lexical/FUNCTIONS.md) owns physical/query profiles;
[bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns SDK conversion and readback.
Do not duplicate their parameter tables here.

Versioned primary sources retained from that inspection:

- [Rust client v1.20.0](https://github.com/qdrant/rust-client/releases/tag/v1.20.0)
- [Text-index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs)
- [Text-query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs)
- [Code tokenizer](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/tokenizers/code.rs)
- [Server index defaults](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/data_types/index.rs)

Compile and execute the exact selected server/client patch with real consumers. The present lock does not supply
this selected path. Query-time k/b and physical index settings remain distinct.

R056 checked the official [Rust quickstart](https://qdrant.tech/documentation/quickstart/) and
[network ports](https://qdrant.tech/documentation/installation/#networking) on 2026-10-28: Rust uses gRPC, ordinarily
port 6334; 6333 is REST/health. Corrected the Windows target example and the bridge context boundary. This is
API/documentation evidence, not a connected ELIOT/server run or a broader version/advisory audit.

<a id="plain-object-publication-donor"></a>
## Windows finalization — exact evidence and limits

Checked on 2026-10-28:

| Source | What it establishes |
|---|---|
| [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info), page updated 2026-05-15 | Documents target naming, RootDirectory and refusal when replacement is disabled. |
| [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) | Documents the handle operation and warns that information-class behavior varies across OS releases. |
| [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew) | Documents access/share compatibility; delete access includes rename. This is not a proof of whole-path stability. |
| [Q&A 6015074](https://learn.microsoft.com/ja-jp/answers/questions/6015074/setfileinformationbyhandle-filerenameinfo-rootdire) | First-hand community report dated 2026-09-25, accepted analysis 2026-09-27: Windows 10.0.26200.0 x64, NTFS, SDK 10.0.26100.0, .NET 10/C# P/Invoke. Relative name plus non-null RootDirectory reportedly returns error 87; absolute name plus null root succeeds. |
| [Report author's linked reproduction](https://github.com/gekka/Win32API_Test_SetFileInformationByHandle) | C#/C++ investigation linked from the report; not executed by this pass. |

The community report is not an official all-Windows support verdict. It records one environment and wrapper
analysis; it does not qualify ELIOT's Rust buffer layout, namespace protection, reopen or crash behavior.

The [root owner](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#windows-finalization) contains the
single selected candidate and required canary. Object storage reuses that seam. R053's private direct-NT finalizer
remains rejected; R054's absolute-target choice remains unverified. Do not describe root→parent handle retention
as proven protection for every absolute-path ancestor or reparse/device-path transition.

Git's same-parent stage/content-address/final-readback pattern is an analogue, not permission to reuse a path-only
finalizer. File-data flush, namespace publication and sudden-power-loss durability remain different claims.

<a id="analog-systems"></a>
## Analogues — patterns, not parallel products

Git loose objects inform immutable publication; Watchman/ignore inform inventory and uncertain-event recrawls;
immutable index builders inform generation visibility/cleanup separation. They do not justify another index,
watcher ledger, backend trait, generic recovery service or simultaneous installation of all donors.

Earlier analogue readsets remain in the R054 snapshot and
[detailed prior register](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).
Qdrant Edge remains a later isolated replacement candidate; reverify its actual released status at evaluation.

<a id="qdrant-update-mode-and-generation-cleanup"></a>
## Mutation and cleanup

[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) owns fixed input, uncertain dispatch,
view installation and whole-generation retirement. Stage 2/3A has no point mutation; Stage 3B is conditional.
Neither profile has physical point cleanup. Readback is not proof that all unresolved requests stopped.

<a id="native-bm25-and-separate-statistics-scope"></a>
## Statistics scope

[Scope resolution](../../crates/search-query/search-access/FUNCTIONS.md) owns base population versus candidate
narrowing. Whole-population selection may omit IDF, but does not erase request-only candidate filters. Top-k bounds
output, not total backend work; measure actual narrow/broad and cold/warm costs.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>
## Markdown OffsetIter

R031 adopted pulldown-cmark 0.13.5 with exact lock and transform identity. Its materializer/unitizer compiler run
is historical evidence; connected parser/materialize/replay behavior remains unrun. The stable anchor is retained
for incoming links; its old wording does not mean the dependency upgrade is still pending.

<a id="typed-configuration-donor-boundaries"></a>
## Typed configuration

Select high-level TOML/Serde with the real decoder. Preserve sparse omission, closed nested structs, native paths,
bounded input and redacted diagnostics. Do not accept dormant fields or expose a large arbitrary backend schema.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>
## Exact matcher

The existing literal scan_chunks helper is not the connected retained reader or a proof of cancellation during
its inner loop. The Exact owner must compile once per request, reset at source boundaries and feed bounded retained
blocks. Raw bytes, decoded text and derived representations are distinct domains.
