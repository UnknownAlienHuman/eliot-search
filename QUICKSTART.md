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

## Index and search

Start the operator-owned Qdrant 1.20.0 service first. Initialize a new application data root once; index opens
an existing current root and does not initialize it. Paths below are placeholders, separate from user originals:

    eliot-search initialize-data-root --data-root "C:\example\data-root"
    eliot-search index "C:\example\repo" --data-root "C:\example\data-root" --qdrant-url http://127.0.0.1:6334
    eliot-search search --query "supersede_unknown_build" --limit 10 --data-root "C:\example\data-root" --qdrant-url http://127.0.0.1:6334

These command forms were executed against isolated native fixtures and the managed repository; the native
operator used its own endpoint. Supply --daemon PATH for a matching binary pair when needed.
Repeat the same index command to refresh a file or tree; no UUID assembly or separate refresh subcommand is
needed. Index respects ignore rules and excludes .git/data-root paths. Link/reparse or incomplete observations
fail; they are not empty inventory. Confirmed in-scope removal/exclusion affects new generations while retained
objects and pinned evidence remain. Sources outside the selected scope are unchanged.

Unchanged native identity plus ObjectRef preserves the logical source head and avoids a new collection.
Search returns verified retained units and is read-only. Current a492442 scoped native evidence passed; earlier
unchanged/key-change/edit cases retain their ff4 scope. R1 is partial: admission is not yet durable for automatic
writable-startup reload, and no resident periodic trigger/interval exists. See the [registry](docs/audit/ANALYSIS_REGISTRY.md#resume).

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

Actual process-kill/power-loss recovery, other platforms and installed-baseline qualification were not run.
The expired-before-spawn process trigger was not executed; that boundary has source review only.
The [registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status) separates earlier native API/pipe probes from
the ad8 recovery/config batch and records the exact checked revisions.

## First persistent path

One explicit file connects through no-execute acquisition, verified plain ObjectRef storage and a guarded
CurrentSourceRecord. prepare-source consumes retained bytes and stores the complete representation/ranges/current
outcome. publish-prepared connects those stored inputs to actual Qdrant/current/evidence/PublishedView.
Read-only search returns verified retained units and citations from that pinned generation. Index now connects
explicit file/tree retention, preparation and publication without manual SourceId assembly. Durable admission,
writable-startup reload, periodic triggering and installed-baseline qualification remain unfinished. Confirmed filesystem deletion excludes membership from
new generations while preserving retained objects and pinned views. Inaccessible or partial inventory is not removal.
