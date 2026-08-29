# Development tools

These utilities are never linked into production binaries.

## Swarm topology validation

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

Checks Cargo/registry/package/assignment identity, exact internal dependencies, cycles/waves, launch
state, line limits and the P00 manifest. JSON output includes SHA-256 for the P00 manifest and every
required contract-pack file.

## Implementation-packet validation

```powershell
pwsh -NoProfile -File tools/validate-implementation-packets.ps1
pwsh -NoProfile -File tools/validate-implementation-packets.ps1 -Json
```

Checks registry-declared `FUNCTIONS.md`, configuration section ownership/packets/example parity,
`search-config` dependencies, secret/autoupgrade floors and the W3 Qdrant qualification/probe/schema
packet.

Both scripts use built-in PowerShell/.NET APIs and create no production dependency. Passing these
structural checks is not runtime, security, Qdrant, performance or product-acceptance evidence.
