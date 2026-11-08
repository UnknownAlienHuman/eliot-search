# Code entrypoints and change map

Current binaries expose root operations, index [PATH], retain-file, prepare-source, publish-prepared, foreground watch, read-only search, Raw/DecodedUtf8 Exact same-session windows, stopped-root collect-objects and archive-export/import. Native binary and library
cases have distinct exact-source scopes in the [registry](../audit/ANALYSIS_REGISTRY.md#evidence-status).
U1 targeted Windows debug acceptance is at c843171, with earlier transport/budget and initial-publication scopes kept separate.
The [owner map](README.md#owners) defines responsibilities; a row does not require keeping a crate.

## Real binaries

| Package / target | Actual entrypoint and implementation |
|---|---|
| eliot-search / eliot-search | [entry.rs](../../bins/eliot-search/src/entry.rs) → [app.rs](../../bins/eliot-search/src/app.rs); [manifest](../../bins/eliot-search/Cargo.toml) |
| eliot-searchd / eliot-searchd | [entry.rs](../../bins/eliot-searchd/src/entry.rs) → [app.rs](../../bins/eliot-searchd/src/app.rs); [manifest](../../bins/eliot-searchd/Cargo.toml) |

Both packages define only their binary target. The former library, main/sealed harness and wave/loopback feature
routes are removed. Check the real binary; package names and retained workspace crates do not imply handlers.

<a id="existing-dispatch-to-replace"></a>
## Current operator dispatch

    CLI entry → app::run_main → native command parse
      standalone help/version → local output, no daemon
      root subcommands        → owned eliot-searchd child with matching --operation and settings
                              → bounded root response validation → output
      watch                   → repeated owned saved-selection Index → cleanup → cancellable interval
      index [PATH]            → same owned child → closed refreshed result → output
      collect-objects         → same owned child → closed marked/verified/deleted/preserved summary
      archive-export          → same owned child → closed complete/generation/objects/control_bytes summary
      archive-import          → same owned child → closed generation/collection/counts summary, optional --retry
      retain-file --file PATH → same owned child → bounded retained-source response validation → output
      prepare-source          → same owned child → parsed source ID/unit-bound stored-unit validation → output
      publish-prepared        → same owned child → closed generation-state validation → output
      session                 → line-oriented SessionClientFrame → OwnedQuerySession → --session-stdio
                              → correlated SessionReply or explicit client-origin refusal → checked finish
      search                  → QueryClient serviced framed child → complete validated terminal → safe output
      exact --raw --literal   → same serviced child with RawExact → validated raw ranges/hex → safe output
      exact --decoded-utf8     → same serviced child with DecodedUtf8 → saved identity refs/ranges/text

    daemon entry → app::run_main → app/bootstrap::parse
      standalone help/version → local output, no config/root acquisition
      root command            → capture startup cwd → bootstrap::resolve → typed RootInvocation
                              → native_root_command::run → search-runtime-owner
      index                   → open current root once → scoped admission or reload all selections
                              → bounded unique file union + small scope metadata → typed retain/prepare
                              → verify scan → guarded scoped removal → publish → refreshed result
      collect-objects         → open_plain_with_recovery/exclusive root → one CONTROL snapshot/ObjectMarks
                              → Prepared binding/embedded Source closure + all Source verification
                              → one streaming final-object sweep → factual summary
      archive-export          → open_plain_with_recovery/exclusive root → verified required object closure
                              → copied CONTROL/objects → independent verification → flush → archive.complete last
      archive-import          → complete archive/served validation → fresh or exactly marked destination guard
                              → selected object copy + install_served_corpus → fresh publication/acceptance
                              → close stores/view → exact marker clear → factual summary
      retain-file             → open current root → source-row capture → safe read → object readback
                              → current-source CAS in search-control-redb → factual result
      prepare-source          → open current root → preserve settings → read-only preparation capture
                              → retained input → prepare/store/read/decode artifact → outcome CAS → stored unit
      publish-prepared        → open root → all-source/preparation capture → fixed pages/evidence
                              → actual backend build/readback + checked canary → current CAS
                              → complete matching RO PublishedView → generation state/counts
      search-stdio            → framed Search/RawExact/DecodedUtf8/Cancel I/O + one joinable synchronous worker
                              → root + bounded pin_published
                              → Search: backend nominations → retained Source/Prepared unit/citation
                              → RawExact: source_inventory → verified Source bytes/ranges/hex, no Qdrant
                              → DecodedUtf8: saved Prepared + Source → verified identity ranges/text
                              → optional QuerySummary from the same RO snapshot → bounded terminal flush with root/view retained
      session-stdio           → session I/O/control + joinable worker owning one fixed window
                              → Search/Raw/DecodedUtf8 Open → original prefix <=64 → flush-committed pages
                              → emitted-item Expand with cached original summary → Close/expiry/EOF → join/release; client observes reap

| CLI subcommand | Daemon flag | Root function |
|---|---|---|
| inspect-data-root | --inspect-data-root | inspect_plain |
| initialize-data-root | --initialize-data-root | initialize_plain |
| check-existing-root-owner | --check-existing-root-owner | open_plain |
| discard-incomplete-data-root | --discard-incomplete-data-root | discard_incomplete_plain |
| watch [--interval-ms N] --qdrant-url URL | owned --index --index-owner-stdio child, no PATH | app/watch.rs → one bounded iteration → observed cleanup → after-completion sleep |
| index [PATH] --qdrant-url URL | --index [--index-path PATH] with matching settings | open_plain_with_recovery → capture_selections/admit_selection → unique scoped/union scan → retain/prepare/remove_current_sources/publish |
| collect-objects [ROOT] | --collect-objects with root settings | collect_objects::run → ObjectMarks::build → inspect_prepared_utf8/Source verification → proven-unmarked sweep |
| archive-export --archive-path OUT [ROOT] | --archive-export --archive-path OUT with root settings | archive_export::run → retained_objects::verify → copy_plain_control/read_object_in → sync_directory → completion marker |
| archive-import --archive-path ARCHIVE --data-root DEST --qdrant-url URL [--retry] | --archive-import with matching settings/retry | archive_import::run → read_served/validate_served_corpus → initialize_plain or open_plain_marked → install_served_corpus/publish → clear_import_marker |
| retain-file --file PATH | --retain-file --source-file PATH | open_plain → acquire_file → store_object/read_object → commit_current_source |
| prepare-source --source-id UUID [--unit N] | --prepare-source --source-id UUID [--unit N] | open_plain → prepared_source::run → retained read/prepare/store/decode → commit_preparation |
| publish-prepared --qdrant-url URL | --publish-prepared with matching settings | open_plain → published_corpus::run → compact references/Prepared hydration/backend/current/view → generation state |
| search --query TEXT [--limit N] --qdrant-url URL | --search-stdio with framed SearchRequest | framed_search::run → pin_published → verified_query::execute → full retained unit/citation |
| exact --raw --literal TEXT --data-root ROOT [--source-id UUID] [--limit N] | --search-stdio with RawExact frame, no endpoint | pin_published → source_inventory → raw_exact::execute → verified raw ranges/hex |
| exact --decoded-utf8 --literal TEXT --data-root ROOT [--source-id UUID] [--limit N] | --search-stdio with DecodedUtf8 frame, no endpoint | same pinned inventory → decoded_exact::execute → verified saved identity ranges/text |
| session --data-root ROOT [--qdrant-url URL] | --session-stdio with SessionClientFrame/SessionReply | CLI search_command::run_session → OwnedQuerySession → framed_search/session + window |

Read-only/root operator functions take (&Path, Instant, &AtomicBool); open_plain_with_recovery takes the shared
Arc cancellation flag for maintained engine repair callbacks. Explicit writable owners such as Index,
collect-objects and archive-export use that capability; marked import uses open_plain_marked with the same guarded
recovery implementation. Ordinary query/open remains non-repairing.
Init/open return PlainRootGuard; inspection returns
PlainRootInspection and discard returns unit. The daemon holds guards through terminal root output.
Its original deadline/cancellation is passed to root/control effects.

The CLI operation comes first and accepts --daemon PATH for child selection. Daemon settings flags may surround
its root command. Positional ROOT or --data-root selects the CLI layer; --config-file and
ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT can supply a missing root. See the
[daemon recipe](../../bins/eliot-searchd/FUNCTIONS.md) for precedence and bounds.

retain-file emits its source row only after bounded same-handle acquisition, immutable object readback, and the
short guarded transaction succeed. It does not produce preparation, Qdrant, search, or evidence results.

prepare-source resolves a retained SourceId without opening the live path. It returns actual complete stored
text/ranges or explicit empty/Gap, using the serialization-derived allowance for a full 64 KiB unit. The CLI
preserves selectors from its first parse; legal native path values may equal option names.

Framed search is connected. It performs no live-source scan, reprepare, publish or CONTROL/backend writes;
only returned retained bindings/objects are read. Publication/recovery keeps complete graph validation.
Root-state output does not report Qdrant availability. Final native qualification remains distinct from this map.

## Work by path

Rows marked future identify integration owners, not installed commands or completed stages.

| Boundary | Source / rule |
|---|---|
| Native CLI and child lifetime | [CLI app](../../bins/eliot-search/src/app.rs), [CLI recipe](../../bins/eliot-search/FUNCTIONS.md) |
| Consumed settings and root dispatch | [bootstrap](../../bins/eliot-searchd/src/app/bootstrap.rs), [root handler](../../bins/eliot-searchd/src/native_root_command.rs) |
| Plain root/control | [root source](../../crates/search-runtime/search-runtime-owner/src/plain_root.rs), [Root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [control source](../../crates/search-control-redb/src/plain_control.rs), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Stable acquisition and retained body | [Reader](../../crates/search-source/search-safe-reader/FUNCTIONS.md), [Objects](../../crates/search-source/search-revision-store/FUNCTIONS.md), daemon native_root_command |
| Current source rows and stale work | [Source family](../../crates/search-source/README.md), [Source values](../../crates/search-source/search-source-registry/FUNCTIONS.md), [Control](../../crates/search-control-redb/FUNCTIONS.md) |
| Retained-only preparation and stored-unit output | [Daemon handler](../../bins/eliot-searchd/src/prepared_source.rs), [CLI validator](../../bins/eliot-search/src/prepared_response.rs), [CONTROL rows](../../crates/search-control-redb/src/current_preparation.rs) |
| Exact UTF-8 and complete artifact/unit ranges | [Materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [Unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Concrete Qdrant operations | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [lexical policy](../../crates/search-lexical/FUNCTIONS.md) |
| Corpus publication/current/view | [Daemon publisher](../../bins/eliot-searchd/src/published_corpus.rs), [CONTROL](../../crates/search-control-redb/src/publication.rs), [RO graph validator](../../crates/search-control-redb/src/publication/verify.rs), [policy](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Stopped-root object collection | [Handler](../../bins/eliot-searchd/src/collect_objects.rs), [CONTROL marks](../../crates/search-control-redb/src/object_marks.rs), [object leaf checks](../../crates/search-source/search-revision-store/src/collect.rs), [owner](../../crates/search-runtime/search-retention/FUNCTIONS.md) |
| Plain archive export | [Handler](../../bins/eliot-searchd/src/archive_export.rs), [shared closure verifier](../../bins/eliot-searchd/src/retained_objects.rs), [CONTROL copy](../../crates/search-control-redb/src/plain_control.rs), [owner](../../crates/search-runtime/search-retention/FUNCTIONS.md#implemented-archive-export-consumer) |
| Served-baseline archive import | [Handler](../../bins/eliot-searchd/src/archive_import.rs), [CONTROL install](../../crates/search-control-redb/src/publication/restore.rs), [marked root](../../crates/search-runtime/search-runtime-owner/src/plain_root.rs), [owner](../../crates/search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport-into-a-fresh-destination) |
| Returned evidence and retained units | [Query worker](../../bins/eliot-searchd/src/verified_query.rs), [CONTROL resolver](../../crates/search-control-redb/src/publication.rs), [evidence rule](../../crates/search-query/search-candidate-validator/FUNCTIONS.md) |
| Published/current query summary | [RO observation](../../crates/search-control-redb/src/publication.rs), [daemon producer](../../bins/eliot-searchd/src/framed_search.rs), [DTO](../../crates/search-provider-protocol/src/dto.rs), [CONTROL rule](../../crates/search-control-redb/FUNCTIONS.md#query-summary-observation) |
| Serviced framed child and presentation | [Daemon I/O](../../bins/eliot-searchd/src/framed_search.rs), [client](../../crates/search-provider-client/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Raw/DecodedUtf8 and same-session windows | [Raw](../../bins/eliot-searchd/src/raw_exact.rs), [Decoded](../../bins/eliot-searchd/src/decoded_exact.rs), [session I/O](../../bins/eliot-searchd/src/framed_search/session.rs), [window](../../bins/eliot-searchd/src/framed_search/window.rs), [owning window recipe](../../crates/search-query/search-continuation/FUNCTIONS.md) |

The old projection-planner/publication source, manifests and exclusive fixtures were retired in P1 with their
VerifiedUnitSet consumers. Their FUNCTIONS.md and AGENTS.md remain canonical Q1 recipes at the existing paths;
their concrete Q1 producer is now the daemon/CONTROL pair above. No old crate was restored.

Use the [caller-search method](WORKING_GUIDE.md#find-callers-before-changing-a-boundary) within the selected owner.
Do not restore deleted daemon implementations or unconsumed APIs because an older map or test named them.

## Keep the implementation small

That read-only query → pinned published evidence → verified retained text/citation → CLI path is connected.
Scoped index, bare selection reload and foreground watch now use those helpers; R1's accepted scopes remain
in the registry. X1 and M1 GC/export/import are DONE only in their selected Windows/Release scopes. A10
QuerySummary is on main2a0462a6 with unchanged Rust source9cfa. Separate Sol CPU/workflow supplements passed;
V1 is DONE for selected Windows/Release ordinary-repo-1 resources (including observed replacement disk),
real-repository workflows and isolated installed pair. Observed replacement high-water was 1,780,153,608 B
<=4 GiB; unsampled actual peak remains UNKNOWN. Ranked coverage remains diagnostic. Retained source9cfa binary
provenance is not a fresh main build or universal platform/peak claim; no READY mandatory row remains.
Entry points and root exclusion are unchanged; no concurrent view framework is added by this documentation.
Confirmed absence and inaccessible inventory stay distinct.
Build the actual owners and delete displaced code with its callers. Root-only command checks, helper-native
probes and retained workspace libraries do not qualify that pipeline or an installed baseline.
