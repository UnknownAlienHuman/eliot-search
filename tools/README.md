# Development tools

These utilities are never linked into production binaries.

## Swarm topology validation

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

Checks Cargo/registry/package/assignment identity, exact internal dependencies, cycles/waves, launch
state, line limits and the P00 manifest.

## Implementation-packet validation

```powershell
pwsh -NoProfile -File tools/validate-implementation-packets.ps1
pwsh -NoProfile -File tools/validate-implementation-packets.ps1 -Json
```

Checks registry-declared `FUNCTIONS.md`, configuration ownership/example parity, `search-config`
dependencies, secret/autoupgrade floors and W3 Qdrant qualification/probe/schema packets.

## W4/W5 packet validation

```powershell
pwsh -NoProfile -File tools/validate-current-packets.ps1
pwsh -NoProfile -File tools/validate-current-packets.ps1 -Json
```

Checks W4 function/qualification registration; W5 reconcile/overlay/code-enricher function links;
launch qualification-path parity; locked currentness/unsaved/no-execute baseline flags; exact unselected
parser identities; and unique mandatory W5 probes.

All scripts use built-in PowerShell/.NET APIs and create no production dependency. Passing structural
checks is not runtime, security, Qdrant, current-workspace, parser, performance or Product Pulse evidence.
