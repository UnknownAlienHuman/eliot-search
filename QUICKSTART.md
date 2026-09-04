# ELIOT Search — current development quickstart

The repository now contains a bootable local shell and a bounded exact-search path.
The current scan mode accepts caller-supplied UTF-8 and deliberately reports
`evidence_bound=false`; it is not yet the W2 retained-revision product claim.

## Build

```powershell
cargo build --locked -p eliot-searchd -p eliot-search
```

## Verify daemon state

```powershell
./target/debug/eliot-searchd.exe --health
./target/debug/eliot-searchd.exe --self-test
./target/debug/eliot-search.exe --version
```

On Linux, omit `.exe`.

## Search piped UTF-8

PowerShell:

```powershell
Get-Content -Raw .\README.md |
  ./target/debug/eliot-search.exe scan-stdin "source"
```

Case-insensitive ASCII comparison:

```powershell
Get-Content -Raw .\README.md |
  ./target/debug/eliot-search.exe scan-stdin-ascii-insensitive "SOURCE"
```

Linux:

```bash
cat README.md | ./target/debug/eliot-search scan-stdin source
```

The CLI launches the sibling `eliot-searchd` binary. Override its location with
`ELIOT_SEARCHD_BIN` or `--daemon PATH`.

## Current truth boundary

Implemented and pushed to `main`:

- canonical contracts, pure domain decisions, and vendor-neutral ports;
- deterministic configuration mechanics;
- runtime-owner, secret lifecycle, control journal, and provider protocol kernels;
- source admission, identity, registry, final-handle read contract, immutable
  encrypted-revision ledger, reconciliation, strict materialization, and
  deterministic unitization;
- bounded exact matching with byte, line, and byte-column coordinates;
- bootable daemon and protocol-only CLI.

Still required before `source_backed_search_available=true`:

- concrete final-handle platform adapter;
- concrete encrypted revision persistence and key handling;
- concrete durable redb backend integration in daemon composition;
- concrete Windows credential/DPAPI adapter;
- persistent authenticated local endpoint and full startup/shutdown orchestration.
