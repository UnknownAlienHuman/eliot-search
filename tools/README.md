# Development tools

These utilities are never linked into production binaries. All repository workflows are manual-only.

## Swarm topology validation

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

Checks Cargo/registry/package/assignment identity, exact internal dependencies, cycles/waves, launch
state, line limits and the P00 manifest.

## All-package function packet coverage

```powershell
pwsh -NoProfile -File tools/validate-function-packets.ps1
pwsh -NoProfile -File tools/validate-function-packets.ps1 -Json
```

Checks exact equality between 45 package entries and the 3 foundation plus 42 package-local primary
function packets, including assignment/wave/write-scope parity and operation-contract structure.

## Stage and later-package read-set validation

```powershell
pwsh -NoProfile -File tools/validate-stage-readsets.ps1
pwsh -NoProfile -File tools/validate-stage-readsets.ps1 -Json
```

Checks:

- exact W0–W10 stage registry and central gate references;
- **68 stage-package assignments** covering all 45 packages at their earliest wave and progressive
  daemon composition through W7;
- W1/W2, W3/W4 and W5/W6 contribution/closure pairs for G1/G2/G3;
- separate `W7_LIFECYCLE` completion receipt before W8/W9;
- **23 later-stage overrides** covering every package assignment after its earliest wave and no
  unnecessary override for an earliest-wave package;
- exact package/wave/base-stage/immediate-prior-stage/write-scope parity across package, function, stage
  and read-set registries;
- prior stage packets replaced by accepted public handoffs;
- exact W7 lifecycle, W8 protocol/daemon/CLI, W9 Product Pulse and W10 activation/scale/evaluation
  supplements;
- W8 standalone CLI receives `bins/eliot-search/W8_CLIENT.md` rather than the W1 packet;
- W10 `search-eval` receives `W10_OPTIONAL_EVALUATION.md` and accepted W9/P15 handoffs rather than W4/W9
  implementation history;
- the `eliot-searchd` feature ladder receives separate W2, W3, W4, W5, W6 and W7 package tickets;
- integration machine packets remain out of ordinary agent context;
- static context count recomputation and sixteen-file ceiling;
- no architecture-master or dependency-implementation reads;
- launch-state links to every machine registry while remaining P00/W0;
- only `search-contracts` authorized and domain/ports conditional;
- root authority/assignment/handoff documentation parity;
- every repository workflow remains read-only and `workflow_dispatch` only.

## P00 draft ticket/control validation

```powershell
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
```

Checks:

- exactly three non-claimable P00 ticket drafts and three unmaterialized context drafts;
- exact package/launch class/write scope/line budgets for contracts, domain and ports;
- unresolved writer/reviewer/base/ticket/context identities and zero claimable records;
- domain/ports remain conditional on an accepted `search-contracts` handoff;
- exact ordered source files, registry selectors and accepted-handoff slots for each context;
- per-source/fragment digest materialization requirements and one writer-visible context artifact;
- no architecture master, implementation source or forbidden control records in draft contexts;
- issued-ticket, materialized-context, lease, submission, review, handoff, supersession and wave-receipt
  directories contain no real records before issuance;
- draft states are excluded from the orchestration state machine;
- `READY → LEASED` requires a new issued ticket and materialized context;
- P00/W0 launch authority remains unchanged;
- all workflows remain manual-only/read-only.

A PASS proves only that drafts are bounded, honest and non-claimable. It does not issue a ticket, create a
writer lease or accept a package.

## Ticket issuance and control-record schema validation

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Json
```

Checks:

- the closed `types-v1.toml` registry has unique names and no unresolved aliases/generic inner types;
- all eight record schemas are registered exactly once and use the registered canonical layout;
- every field kind resolves to a built-in or named type;
- `path[]` kinds are element types rather than accidental list-of-list declarations;
- field paths/orders are unique and canonical orders are contiguous from one;
- every generic closed enum has an exact equality/allowed-set rule;
- embedded `signature.record_sha256` keeps signed-payload semantics while exact full-file digest remains
  external;
- package handoff records use unique `handoff-id` paths instead of API digest paths;
- canonicalization, issuance/recovery operations and orchestration wiring remain present;
- issued control directories remain zero-state;
- the dedicated workflow remains read-only and `workflow_dispatch` only.

A PASS is schema closure only. It is not materialization, issuance, runtime, package, gate or wave
evidence.

## Implementation-packet validation

```powershell
pwsh -NoProfile -File tools/validate-implementation-packets.ps1
pwsh -NoProfile -File tools/validate-implementation-packets.ps1 -Json
```

Checks legacy `swarm/crates.toml` function links, configuration ownership/example parity,
`search-config` dependencies, secret/autoupgrade floors and W3 Qdrant packets.

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

Checks the complete W5 cross-contract, three owner packets, stage settings and finite bounds, 42
currentness/overlay probes, 17 unselected Rust parser probes, exact G3 W5/W6 evidence partition,
package-local write scopes and manual-only workflow wiring.

## W6 proof packet validation

```powershell
pwsh -NoProfile -File tools/validate-proof-packets.ps1
pwsh -NoProfile -File tools/validate-proof-packets.ps1 -Json
```

Checks resolver/comparator/exact links, P00/W0 launch preservation, locked ambiguity/non-normative/
frozen-denominator rules, unselected regex/structural profiles, 52 mandatory probes and G3 evidence.

## W7 lifecycle validation

```powershell
pwsh -NoProfile -File tools/validate-w7-lifecycle.ps1
pwsh -NoProfile -File tools/validate-w7-lifecycle.ps1 -Json
```

Checks restrictive-security, retention, mark/sweep, purge, restore, handle/continuation/candidate,
publication/reclaim and receipt-separation contracts. Evidence remains unexecuted.

## W8 client-edge validation

```powershell
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1 -Json
```

Checks generic-edge ownership, recipe closure, authority boundaries, locked settings, standalone client
contract, 50 probe states, G4 mapping and blocked/unqualified status.

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

Checks three unselected candidate profiles; nine package/integration/evaluation ownership packets;
model/document worker, daemon, scale and `search-eval` candidate-evaluation contracts; 45 disabled probe
templates; candidate-specific five-ID G6 maps; locked content/migration/removal settings; manual-only
workflows; and unchanged P00/W0 authority.

Passing any structural validator is not runtime, Windows-security, Qdrant, current-workspace, parser,
comparison, exact-proof, Product Pulse, provider, optional-depth, package, wave or gate evidence.
