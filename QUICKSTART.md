# Quickstart status

The four root operator commands are qualified for Windows debug artifacts with Rust 1.98.0 at source
ad8cc9e8fa498d43b644246af3b7f4900890317a. S1 final Windows debug checks, strict Clippy, builds and basic retained-file
execution passed at 06bb6877487d7e3caac9d955be855c86dec1a1b5. These remain separate qualification scopes.
P1 adds retained-only preparation: core native cases ran with 88e00ef binaries; the d2ccb596 CLI passed the
focused request-binding regressions and a nonzero unit selection using the unchanged daemon/libraries.
Q1 at 3c33d831 passed actual Windows debug CLI publication/current/evidence/view qualification with Qdrant 1.20.0.
U1 now connects read-only search returning verified full retained units and citations. Targeted final-pin cases
passed at c843171; initial Unicode publications at 20c31 and earlier 2ae transport/budget cases retain their
separate scopes. This is Windows debug qualification, not an installed baseline or the complete product.
See the [exact source, artifacts and verification scope](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
A10's bounded query header landed on main2a0462a6 with Rust equal to source9cfa; measured binaries retain their
9cfa build provenance, not a fresh main build. Scoped Windows/Release scale/isolated installed observations and
separate CPU/workflow and observed replacement disk results close V1 for the selected Windows/Release
ordinary-repo-1, real-repository workflows and isolated installed scope. Actual unsampled peaks, cold-cache
and other-platform qualification are not claimed; this is not a fresh main build.

## Index and search

The qualified ordinary-repo-1 measurements use optimized RELEASE, not target/debug or default cargo run.
From the repository root build both sibling executables:

    cargo +1.98.0 build --release --locked -p eliot-search -p eliot-searchd --bins

The commands below explicitly select that release pair. If Cargo's target directory is configured elsewhere,
replace .\target with that actual directory; retain the release subdirectory and matching --daemon path.
Other bare executable examples below describe command syntax, not DEBUG performance qualification.

Start the operator-owned Qdrant 1.20.0 service first. Initialize a new application data root once; index opens
an existing current root and does not initialize it. Paths below are placeholders, separate from user originals:

    .\target\release\eliot-search.exe initialize-data-root --data-root "C:\example\data-root" --daemon .\target\release\eliot-searchd.exe
    .\target\release\eliot-search.exe index "C:\example\repo" --data-root "C:\example\data-root" --qdrant-url http://127.0.0.1:6334 --daemon .\target\release\eliot-searchd.exe
    .\target\release\eliot-search.exe index --data-root "C:\example\data-root" --qdrant-url http://127.0.0.1:6334 --daemon .\target\release\eliot-searchd.exe
    .\target\release\eliot-search.exe search --query "supersede_unknown_build" --limit 10 --data-root "C:\example\data-root" --qdrant-url http://127.0.0.1:6334 --daemon .\target\release\eliot-searchd.exe

These command forms were executed against isolated native fixtures and the managed repository; the native
operator used its own endpoint. Supply --daemon PATH for a matching binary pair when needed.
Each CLI invocation is a new process. index PATH persists a File/Tree admission and refreshes only that scope;
bare index reloads all stored selections and refreshes their deduplicated union. Explicit File admission wins
over Tree ignore rules. Filesystem deletion keeps admission and its known kind for later re-add.
Protected .git/data-root paths stay excluded/refused; File precedence does not override that protection.
Tree walks respect ignore rules. Link/reparse or incomplete observations fail.
Only confirmed in-scope absence removes current membership; an ignored but still-present file is not removed.
Retained objects/pinned evidence remain. Sources outside an explicit PATH refresh are unchanged.

A legacy root has no admissions by default. Bare index returns INDEX_SELECTION_REQUIRED; retain-file rows are
not silently promoted into selections. Nor are unrelated legacy rows implicitly prepared or omitted: publication
still returns PUBLICATION_SOURCE_NOT_PREPARED if any current row lacks matching preparation.

Unchanged native identity plus ObjectRef preserves the logical source head and avoids a new collection.
Search returns verified retained units and is read-only. ec9fb6f selection-reload source/native checks passed
in the scopes recorded in the [registry](docs/audit/ANALYSIS_REGISTRY.md#resume); earlier cases retain their own
binary scope. Later query/window and maintenance scopes are recorded in the registry; they are not pending
merely because the older evidence above used another source revision.

## Watch admitted selections

After initializing the root and admitting at least one file/tree with index PATH, run:

    .\target\release\eliot-search.exe watch --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\data-root" --daemon .\target\release\eliot-searchd.exe

Watch accepts no PATH. It reloads saved selections in each owned Index child. The default interval is 30000 ms;
--interval-ms N selects a positive interval. The interval starts after child/pipe cleanup and report output,
so slow indexing does not overlap the next iteration. Each iteration has its own original 120-second budget.
Ordinary iteration errors are reported before the next wait; fatal cleanup/signal/output failure ends watch.
Ctrl-C stops watch; native sleep/active cases exited 130 without surviving Index children.

Each child retains at most 64 KiB of stdout and stderr while continuing to drain excess. The internal owner
stdin closes when its parent disappears, requesting cooperative Index cancellation. This is foreground work,
not an OS service. Qdrant remains operator-owned. Search may run separately and remains read-only.

Explicit writable owners such as Index and collect-objects use guarded CONTROL crash recovery when NeedsRepair is observed. It validates a
recovered owned temporary copy before writes to the unchanged original, then uses maintained redb recovery,
clean close and inspection. Read-only open/query do not repair. Unsupported/corrupt or missing-format storage
is refused without reset or copying candidate bytes back. See the [recovery owner](crates/search-runtime/search-runtime-owner/FUNCTIONS.md#guarded-writable-open)
and [exact native scopes](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

## Root operator commands

Use the matching CLI/daemon debug pair. The examples assume those executables are available to the shell.
Replace C:\example paths with your selected application-owned root locations; they are placeholders.

    eliot-search --help
    eliot-search --version
    eliot-search inspect-data-root "C:\example\root"
    eliot-search initialize-data-root "C:\example\root"
    eliot-search check-existing-root-owner "C:\example\root"
    eliot-search discard-incomplete-data-root "C:\example\incomplete-root"

The corresponding direct daemon commands are:

    eliot-searchd --help
    eliot-searchd --version
    eliot-searchd --inspect-data-root "C:\example\root"
    eliot-searchd --initialize-data-root "C:\example\root"
    eliot-searchd --check-existing-root-owner "C:\example\root"
    eliot-searchd --discard-incomplete-data-root "C:\example\incomplete-root"

Initialization has no external initialization ID and refuses an already-current root. Discard requires a
recognized, empty unfinished initialization; it does not reset a current root or remove unknown/user data.
The stable root.lock is retained. Check-existing-owner opens the root under exclusion, reports its state
and releases the guard; it does not start a search service.

The CLI selects its child using --daemon PATH, then ELIOT_SEARCHD_BIN, then a sibling daemon or PATH.
For an explicit matching pair:

    eliot-search inspect-data-root --data-root "C:\example\root" --daemon "C:\example\bin\eliot-searchd.exe"

## Retain one file

Use an already-initialized current root and an explicit local file; paths below are placeholders.

    eliot-search retain-file --file "C:\example\input.txt" --data-root "C:\example\root"
    eliot-searchd --retain-file --source-file "C:\example\input.txt" --data-root "C:\example\root"

The 8 MiB operation returns source identity, revisions, native file identity and the verified ObjectRef after
the current source row commits. It does not return a prepared artifact, index, query or excerpt.
[Source ownership](crates/search-source/README.md) defines the retained-reader and stale-CAS boundaries;
the [registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status) separates the final 06bb gate/basic run from earlier native scopes.

## Prepare a retained source

Use the source_id returned by retain-file and the same current root:

    eliot-search prepare-source --source-id UUID --data-root "C:\example\root"
    eliot-search prepare-source --source-id UUID --unit 1 --data-root "C:\example\root"
    eliot-searchd --prepare-source --source-id UUID --data-root "C:\example\root"

Replace UUID with that actual source_id. Unit ordinals start at zero; omission selects unit 0. The command
returns the stored artifact/input/settings/revisions and a complete selected unit with text, byte ranges and
logical-line bounds. An empty artifact has zero units. Binary/invalid UTF-8 returns an explicit bound Gap
with no artifact/unit. An unavailable ordinal, missing/corrupt retained object or stale completion is an error.

Preparation uses retained objects even after the original path changes or disappears and preserves already
selected settings. Native proof changed/deleted only private scratch inputs; deleting user originals is not
a setup step. The 64 KiB complete-unit ceiling is preserved, with 397312 bytes allowed for escaped JSON and
metadata. This returns stored text, not Qdrant search results or citations.
[Daemon](bins/eliot-searchd/FUNCTIONS.md#prepare-source-path) and
[unitizer](crates/search-prep/search-unitizer/FUNCTIONS.md) describe the actual path.

## Publish the prepared corpus

Start the selected same-host Qdrant 1.20.0 service as its operator, with separately owned storage. The ordinary
gRPC endpoint is port 6334; the application neither starts nor stops that service. Use a current application
root whose selected sources already have matching preparation outcomes:

    eliot-search publish-prepared --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\root"
    eliot-searchd --publish-prepared --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\root"

If that operator service requires an existing key, supply ELIOT_QDRANT_API_KEY in the environment; never pass
it in argv. Publication reads retained artifacts only. Deleted-original qualification uses private fixtures,
not a requirement to remove user input.

The result reports exact collection/generation and complete source/Gap/point/page counts. It accepts no
--query or --limit. Readiness uses an independent checked saved-point canary; publication stores compact
reference pages and hydrates actual upload/readback text from immutable Prepared objects.
Missing preparation/backend/current data, corrupt evidence and unresolved effects remain errors, not empty success.

The Windows native run used a fresh 48-character operator storage root. Controlled 76/49-character storage-path
cases showed depth sensitivity during Gridstore index creation; this is not a universal MAX_PATH threshold or
an application path/name/profile workaround. See [operator storage](crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage)
and [exact evidence](docs/audit/ANALYSIS_REGISTRY.md#evidence-status). The original failed root remains preserved
with an unresolved target; do not reset it to claim recovery.

## Search retained text

    eliot-search search --query "identifier" --limit 10 --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\root"

Search opens the committed generation read-only and preserves Qdrant order. Each returned card contains the
complete stored unit, score and citation: generation/point, namespace/source, Source/Prepared ObjectRefs,
ordinal and byte/line ranges. The original file may be changed or absent; search neither opens it nor prepares
or publishes anything. In native qualification only owned scratch inputs were deleted; do not remove user files.

Queries contain 1..4096 UTF-8 bytes without NUL. Limit defaults to 10 and is 1..64. A unit is at most 64 KiB;
the entire framed JSON terminal is at most 8 MiB, including its length prefix. OutputLimit retains whole cards
and reports the omitted tail without clipping text. Missing returned objects produce explicit gaps; corruption
is an error. Unreturned sources are not scanned or read. No result is an exhaustive negative proof.

The CLI uses one owned framed daemon child; root/view guards remain live through terminal flush, and the client
observes child exit/reap. CONTROL and backend collections are unchanged by search. See the
[query evidence rule](crates/search-query/search-candidate-validator/FUNCTIONS.md) and
[wire bounds](crates/search-provider-protocol/FUNCTIONS.md#framing-and-bounded-output).

## Query scope header

Current Search/Raw/DecodedUtf8 CLI results include Terminal.summary; session pages use WindowSummary.corpus.
It reports published wanted revision, published/selected source and preparation-gap counts, then either Known
CONTROL wanted/lag or a typed Unavailable reason. Known CONTROL lag is not live-filesystem freshness; a metadata-
only Unavailable observation does not discard a valid result. Actual evidence corruption still fails. Raw may
prove a complete negative despite a published preparation gap because it verifies undecoded retained Source.
Session pages/Expand retain the original cached summary. There is no new CLI flag or implicit refresh.

The isolated candidate install ran initialize, index PATH, bare index, Search, Raw, Decoded and collect-objects
with all seven commands exiting 0, outside the checkout and without --daemon. Actual default sibling path/hash
was verified. Separate CPU/workflow and observed replacement disk results complete the selected V1 scope without
replacing the canonical performance baseline. Replacement observed high-water was 1,780,153,608 B <=4 GiB;
post-cleanup backend was 839,792,099 B <=2 GiB, with unsampled peak UNKNOWN. These measurements are not cold-cache
or cross-platform proof. Exact source9cfa scopes/limits are in the
[checkpoint](docs/audit/ANALYSIS_REGISTRY.md#a10-v1-candidate).

## Raw literal Exact

Use the matching RELEASE siblings built above and an existing committed published inventory:

    .\target\release\eliot-search.exe exact --raw --literal "needle" --data-root "C:\example\data-root" --limit 10 --daemon .\target\release\eliot-searchd.exe

Add --source-id UUID to scan one published source; omission scans all published sources, including Empty and
PreparationGap inputs without points. The command itself needs no Qdrant endpoint/server and never publishes,
prepares or opens live source paths. An initialized but unpublished root is not an empty successful scan.

TEXT is a nonempty UTF-8 literal of at most 4096 bytes, matched as bytes over UNDECODED retained Source.
Results bind generation/source/revisions/ObjectRef, raw byte ranges and exact bytes_hex. Missing Source objects
produce explicit gaps; corrupt bytes fail. --limit N allows 1..64 whole emitted match/gap items without ending
the scan. OutputLimit and scan_complete are separate; complete_negative needs every selected input verified/
scanned, zero matches and no missing input. The total frame remains 8 MiB.

This RAW slice is qualified at its recorded source/native scope, not an R1 resource-profile rerun or all
of X1. Retained DecodedUtf8 is available separately below. Extracted has no real producer/implemented route;
arbitrary non-UTF-8 needles remain unsupported. The connected session/window consumer and its selected
qualification scope are recorded by the window owner and registry. See the
[Exact recipe](crates/search-query/search-exact/FUNCTIONS.md#implemented-raw-slice) and
[registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

## Exact over retained DecodedUtf8

    .\target\release\eliot-search.exe exact --decoded-utf8 --literal "Café" --data-root "C:\example\data-root" --limit 10 --daemon .\target\release\eliot-searchd.exe

Exactly one of --raw/--decoded-utf8 is required. This mode reads the actual SAVED Prepared UTF-8 artifact from
the same pinned published inventory and verifies its Source plus captured settings/binding. It does not decode
or materialize input on demand, normalize BOM/CRLF, reprepare missing artifacts or use live/backend fallback.
The identity map is verified by full retained source/representation byte equality. Matching scans full text
across preparation-unit boundaries. Results contain both ObjectRefs, source/representation byte ranges and text.

PreparationGap returns PREPARATION_GAP with no artifact ref. Missing Ready input/artifact returns an explicit
gap retaining the selected PreparedRef; corruption is fatal. Gaps prevent complete negative. Output limits
remain presentation cuts: UTF-8 literal <=4096 bytes, at most 64 whole items, 8 MiB total frame. Raw matching
remains available when a Prepared representation is absent. Same-session Continue/Expand/expiry are connected;
see the [window owner](crates/search-query/search-continuation/FUNCTIONS.md#verification-scope) for their bounded
Windows/Release qualification. Extracted still has no real producer/route.

## Export a plain archive

Use the matching RELEASE siblings built above and a current root with a served generation:

    .\target\release\eliot-search.exe archive-export --archive-path "C:\example\archives\snapshot-1" --data-root "C:\example\data-root" --daemon .\target\release\eliot-searchd.exe

The output must be fresh, outside/non-overlapping with the source, under an existing non-reparse parent.
Export takes exclusive root ownership; close sessions first. It verifies the required retained closure, copies
CONTROL/objects and independently verifies the copy before publishing archive.complete last. It makes no
Qdrant connection and copies no backend storage. The archive is deliberately non-runnable, including after
interruption; it is not a cloned installation. Import uses the separate fresh-destination command below; see the
[archive owner](crates/search-runtime/search-retention/FUNCTIONS.md#implemented-archive-export-consumer) and
[recorded qualification scopes](docs/audit/ANALYSIS_REGISTRY.md#core-corrections-and-archive-export).

## Import the served archive into a fresh destination

Start the operator-owned Qdrant service and use the matching RELEASE siblings. DEST must be a fresh,
non-overlapping path under an existing non-reparse parent; do not initialize it separately:

    .\target\release\eliot-search.exe archive-import --archive-path "C:\example\archives\snapshot-1" --data-root "C:\example\imported-root" --qdrant-url http://127.0.0.1:6334 --daemon .\target\release\eliot-searchd.exe

The complete archive is verified before destination effects. Only the whole SERVED inventory is installed,
with its saved Source/preparation facts and explicit Empty/Gap outcomes. Current unserved desired rows and
admissions are not imported. Native file keys are historical; no live-file read or hidden preparation occurs.
The destination publishes with fresh backend identity and becomes usable only after verified current and marker
removal. If interrupted, repeat this same command with --retry and the same ARCHIVE/DEST. Wrong bindings and
populated markerless destinations refuse; preserve the partial destination rather than resetting it.

M1 GC/export/import retains its recorded Windows/Release scopes. V1's selected ordinary-repo-1, workflow and
isolated installed scope is complete; universal platform/peak qualification is not implied. See the [import owner](crates/search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport-into-a-fresh-destination)
and [exact evidence](docs/audit/ANALYSIS_REGISTRY.md#m1-import-delivery).

## File, environment and CLI settings

An explicit UTF-8 TOML file at C:\example\settings\operator.toml may contain:

    data_root = 'relative-root'

Read that file with either executable:

    eliot-search inspect-data-root --config-file "C:\example\settings\operator.toml"
    eliot-searchd --config-file "C:\example\settings\operator.toml" --inspect-data-root

That file selects C:\example\settings\relative-root. Environment and CLI relative roots instead use captured
startup cwd. For example, in PowerShell:

    $env:ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT = 'environment-root'
    eliot-search inspect-data-root --config-file "C:\example\settings\operator.toml"
    eliot-search inspect-data-root --config-file "C:\example\settings\operator.toml" --data-root "cli-root"
    Remove-Item Env:ELIOT_SEARCH__BOOTSTRAP__DATA_ROOT

Precedence is file < environment < CLI. A positional ROOT and --data-root are alternatives; one layer must
supply a root. Native CLI/environment paths are supported without lossy conversion. Empty or escaped-NUL file
values still fail before root effects even when a valid higher layer would override them. Unknown/duplicate
fields and wrong types fail. Only data_root is consumed; there is no generic --set or diagnostic_limit setting.

## Qualification limits

Recovery/config execution covered 35 native command records and 22 assertions, including real vendor
preformat-empty retry/discard, stable lock identity, recognized partial stages and preservation of sentinels.
Stage states were constructed from actual producer bytes and real hardlinks, not produced by process kills.
The savepoint regression preserves the same golden savepoint ID 1 and retained user data on refusal.

The earlier root-only batch did not run process-kill/power-loss recovery, other platforms or installed-baseline
qualification. Later watch parent-death and dirty-engine cases have separate scopes in the registry; power loss
and the exact pre-spawn signal boundary remain NOT_RUN.
The [registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status) separates earlier native API/pipe probes from
the ad8 recovery/config batch and records the exact checked revisions.

## First persistent path

One explicit file connects through no-execute acquisition, verified plain ObjectRef storage and a guarded
CurrentSourceRecord. prepare-source consumes retained bytes and stores the complete representation/ranges/current
outcome. publish-prepared connects those stored inputs to actual Qdrant/current/evidence/PublishedView.
Read-only search returns verified retained units and citations from that pinned generation. Index now connects
explicit file/tree retention, preparation and publication without manual SourceId assembly. Admission is durable,
and bare index reloads it at writable startup. Foreground periodic watch is connected; V1's selected installed
scope is complete while universal peaks and other platforms remain unclaimed. Confirmed filesystem deletion excludes membership from
new generations while preserving retained objects and pinned views. Inaccessible or partial inventory is not removal.
