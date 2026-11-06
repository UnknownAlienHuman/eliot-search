# Quickstart status

The four root operator commands are qualified for Windows debug artifacts with Rust 1.98.0 at source
ad8cc9e8fa498d43b644246af3b7f4900890317a. S1 final Windows debug checks, strict Clippy, builds and basic retained-file
execution passed at 06bb6877487d7e3caac9d955be855c86dec1a1b5. This is not an installed baseline or a connected Search product.
P1 adds retained-only preparation: core native cases ran with 88e00ef binaries; the d2ccb596 CLI passed the
focused request-binding regressions and a nonzero unit selection using the unchanged daemon/libraries.
Q1 at 3c33d831 passed actual Windows debug CLI publication/current/evidence/view qualification with Qdrant 1.20.0.
It returns backend nominations and stored range metadata; read-only verified excerpt search remains U1.
See the [exact source, artifacts and verification scope](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

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
    eliot-search publish-prepared --qdrant-url http://127.0.0.1:6334 --query "identifier" --limit 10 --data-root "C:\example\root"
    eliot-searchd --publish-prepared --qdrant-url http://127.0.0.1:6334 --query "identifier" --data-root "C:\example\root"

If that operator service requires an existing key, supply ELIOT_QDRANT_API_KEY in the environment; never pass
it in argv. Publication reads retained artifacts only. Deleted-original qualification uses private fixtures,
not a requirement to remove user input.

The result reports exact collection/generation, complete source/Gap/point/page counts and optional SDK
point/evidence metadata. Readiness uses a separate checked saved-point canary; the user query may have no hits.
Missing preparation/backend/current data, corrupt evidence and unresolved effects remain errors, not empty success.
This command does not emit retained excerpts or citations.

The Windows native run used a fresh 48-character operator storage root. Controlled 76/49-character storage-path
cases showed depth sensitivity during Gridstore index creation; this is not a universal MAX_PATH threshold or
an application path/name/profile workaround. See [operator storage](crates/search-runtime/search-runtime-owner/FUNCTIONS.md#operator-qdrant-storage)
and [exact evidence](docs/audit/ANALYSIS_REGISTRY.md#evidence-status). The original failed root remains preserved
with an unresolved target; do not reset it to claim recovery.

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
Next U1 returns a verified retained search excerpt through a read-only command. The
[one-file product path](docs/implementation/README.md#2-walking-skeleton--one-file-to-one-cli-result)
and installed-baseline qualification remain unfinished.
