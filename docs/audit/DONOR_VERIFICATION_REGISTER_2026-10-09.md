# Donor evidence register

Inspected APIs, selected design, installed dependencies and executed behavior are separate facts.
Update the affected row with source/version/features/date when code adopts it; do not scan every donor each task.
[Previous register](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
preserves the detailed earlier investigations. They are not new runs or current release claims.

<a id="installed-selected-and-optional"></a>
<a id="typed-configuration-donor-boundaries"></a>
## Main dependency baseline

At main `6ae24197` the source/manifests still precede the unmerged #610 cutover:

| Area | Installed / selected distinction |
|---|---|
| Qdrant client | Main lock 1.19.0; scoring-text 1.20.x target still requires a coordinated real consumer upgrade |
| redb | Main lock 2.6.3; typed metadata/snapshots, not searchable bodies |
| Tokio | Main lock 1.53.1; select actual I/O/process features when wiring consumers |
| Markdown | Main pin/lock 0.13.1; branch 0.13.5 and its historical narrow Clippy are not main implementation |
| XML / literal | Main quick-xml 0.42.0 / aho-corasick 1.1.5; use existing bounded consumers |
| TOML | Main toml_parser 1.1.4+spec-1.1.0 is the legacy config mechanism; typed TOML/Serde adoption happens with its consumer |
| Unicode / globs | Main unicode-ident 1.0.26, unicode-normalization 0.1.25, globset 0.4.20 |

Do not interpret transitive crate presence as a connected feature. At adoption inspect exact API, features,
MSRV, license, advisories and artifact requirements; regenerate the lock with the consuming code, not in this docs cut.

<a id="plain-object-publication-donor"></a>
## Local lock and no-clobber publication — R062

Primary documentation read 2026-10-29:

| Source | Selected use and limits |
|---|---|
| [std File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock), stabilized in 1.89 | One cooperating-process root lock; retain the same opened lock file. Platform semantics are not hostile-user protection. |
| [tempfile 3.27.0 NamedTempFile](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist_noclobber) | Private final-parent stage and no-overwrite publication; may leave a stage link, is path-based, and does not establish universal crash durability. |

[Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md#shared-no-clobber-publication) owns the selected
algorithm. Pin tempfile with actual code. No dependency/source change or compiler/platform run occurred here.
The old custom Win32/NT finalizer and ancestor-lock investigation is historical: the explicit one-account private-
directory scope does not promise resistance to a hostile process with the same OS privileges.

<a id="qdrant-120-direct-bm25"></a>
## Qdrant scoring text

[Lexical](../../crates/search-lexical/FUNCTIONS.md) owns text/physical/query profiles;
[bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns SDK mapping and readback.
Earlier primary references: [client v1.20.0](https://github.com/qdrant/rust-client/releases/tag/v1.20.0),
[text-index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs),
[text-query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs).
These are retained inspection links, not reverified releases in R062. Rust gRPC normally uses 6334, not REST 6333;
use the configured same-host endpoint. Compile/run the exact adopted client/server together at implementation.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>
## Markdown OffsetIter

The R031 parser correction and narrow historical compiler run belong to the unmerged branch. Main still uses
0.13.1. Follow the [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction)
and immutable prior register; do not call old evidence a current-main or end-to-end run.

<a id="analog-systems"></a>
<a id="qdrant-update-mode-and-generation-cleanup"></a>
<a id="native-bm25-and-separate-statistics-scope"></a>
## Other owner evidence

[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) owns mutation uncertainty and whole-
generation cleanup; [selection](../../crates/search-query/search-access/FUNCTIONS.md) owns candidate versus IDF scope.
[Git acquisition](../../crates/search-source/search-safe-reader/FUNCTIONS.md#concrete-git-path) retains the controlled-
child recipe and R060's narrowly executed hook diagnostic. [Client](../../crates/search-provider-client/FUNCTIONS.md)
retains Tokio I/O evidence. Re-read only the source relevant to a changed assumption.
Application crypto/key-management donors are not selected under [local scope](../architecture/LOCAL_SCOPE.md).
