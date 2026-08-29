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

## W4/W5 packet validation

```powershell
pwsh -NoProfile -File tools/validate-current-packets.ps1
pwsh -NoProfile -File tools/validate-current-packets.ps1 -Json
```

Checks W4 function/qualification registration; W5 reconcile/overlay/code-enricher function links;
launch qualification-path parity; locked currentness/unsaved/no-execute baseline flags; exact unselected
parser identities; and unique mandatory W5 probes.

## W6 proof packet validation

```powershell
pwsh -NoProfile -File tools/validate-proof-packets.ps1
pwsh -NoProfile -File tools/validate-proof-packets.ps1 -Json
```

Checks resolver/comparator/exact function and qualification links, P00/W0 launch preservation, locked
ambiguity/non-normative/frozen-denominator rules, unselected regex/structural profiles, 52 unique
mandatory `UNAVAILABLE` probes and exact G3 evidence IDs.

## W8 client-edge validation

```powershell
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1 -Json
```

Checks generic-edge ownership, exact recipe closure, authority boundaries, locked client/optional-profile
settings, 50 generic/optional probe states, G4 mapping and blocked/unqualified status.

## W9 Product Pulse validation

```powershell
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1 -Json
```

Checks the single `search-eval` package owner plus integration/reviewer roles; 49 mandatory corpus cases;
33 metric definitions; Architecture SLO values; 60 mandatory `UNAVAILABLE` probes; exact six-ID G5 map;
locked fairness/privacy/verdict settings; manual-only CI; and unchanged P00/W0 launch/optional-depth state.

Passing structural checks is not runtime, security, Qdrant, current-workspace, comparison, exact-proof,
Windows-performance or Product Pulse acceptance evidence.
