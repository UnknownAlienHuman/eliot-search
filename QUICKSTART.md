# Quickstart status

The four root operator commands are qualified for Windows debug artifacts with Rust 1.98.0 at source
ad8cc9e8fa498d43b644246af3b7f4900890317a. This is not an installed baseline or a connected Search product.
See the [exact source and verification scope](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

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

Source acquisition, plain object retention/current rows, preparation, Qdrant publication and verified search
remain unconnected. Next is one bounded, no-execute file acquisition → plain ObjectRef → guarded CurrentSourceRecord
in the same control database. The later [one-file path](docs/implementation/README.md#2-walking-skeleton--one-file-to-one-cli-result)
must connect preparation, Qdrant and a retained excerpt through real callers. Root qualification does not
complete that goal.
