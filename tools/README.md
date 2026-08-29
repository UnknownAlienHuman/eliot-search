# Development tools

These utilities are never linked into production binaries. All repository workflows are manual-only
unless the owner deliberately changes that policy.

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

## W4 registry and W5 qualification validation

```powershell
pwsh -NoProfile -File tools/validate-current-packets.ps1
pwsh -NoProfile -File tools/validate-current-packets.ps1 -Json
```

Checks W4 function/qualification registration; W5 function links; launch qualification-path parity;
locked currentness/unsaved/no-execute baseline flags; and 42 mandatory W5 probes.

## W5 deep current-workspace validation

```powershell
pwsh -NoProfile -File tools/validate-w5-current.ps1
pwsh -NoProfile -File tools/validate-w5-current.ps1 -Json
```

Checks the complete W5 cross-contract, 3 owner packets, stage settings and finite bounds, 42 currentness/
overlay probes, 17 unselected Rust parser probes, exact G3 W5/W6 evidence partition, package-local write
scopes and manual-only workflow wiring.

## W6 proof packet validation

```powershell
pwsh -NoProfile -File tools/validate-proof-packets.ps1
pwsh -NoProfile -File tools/validate-proof-packets.ps1 -Json
```

Checks resolver/comparator/exact links, P00/W0 launch preservation, locked ambiguity/non-normative/
frozen-denominator rules, unselected regex/structural profiles, 52 mandatory probes and G3 evidence.

## W8 client-edge validation

```powershell
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1 -Json
```

Checks generic-edge ownership, recipe closure, authority boundaries, locked settings, 50 probe states,
G4 mapping and blocked/unqualified status.

## W9 Product Pulse validation

```powershell
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1 -Json
```

Checks Product Pulse roles, 49 corpus cases, 33 metrics, S30 targets, 60 mandatory probes, six-ID G5
map, locked fairness/privacy/verdict settings and manual-only CI.

## W10 optional-depth validation

```powershell
pwsh -NoProfile -File tools/validate-w10-optional-depth.ps1
pwsh -NoProfile -File tools/validate-w10-optional-depth.ps1 -Json
```

Checks three unselected candidate profiles; eight one-package/integration ownership packets; model and
document worker function contracts; 45 disabled probe templates; candidate-specific five-ID G6 maps;
locked gate/content/migration/removal settings; optional-profile defaults; manual-only workflows; and
unchanged P00/W0 launch authority.

Passing structural checks is not runtime, security, Qdrant, current-workspace, parser, comparison,
exact-proof, Windows-performance, Product Pulse, provider or optional-depth acceptance evidence.
