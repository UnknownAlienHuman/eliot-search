# Development and structural-validation tools

These utilities are repository-development tools only. They are never linked into production binaries,
do not issue control records and do not create package, gate or wave acceptance.

Every workflow under `.github/workflows/` is `workflow_dispatch`-only, declares `contents: read` and
disables checkout credential persistence. A workflow run is structural evidence only.

## Core topology

### `validate-swarm.ps1`

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

Checks Cargo/registry/package/assignment identity, exact internal dependency parity, graph cycles/waves,
launch state, line limits and the P00 contract manifest.

### `validate-function-packets.ps1`

```powershell
pwsh -NoProfile -File tools/validate-function-packets.ps1
pwsh -NoProfile -File tools/validate-function-packets.ps1 -Json
```

Checks exact equality between all 45 packages and their three foundation plus 42 package-local primary
function packets, including assignment/wave/write-scope parity and operation-contract structure.

### `validate-stage-readsets.ps1`

```powershell
pwsh -NoProfile -File tools/validate-stage-readsets.ps1
pwsh -NoProfile -File tools/validate-stage-readsets.ps1 -Json
```

Checks W0–W10 stage composition, 68 stage-package assignments, 23 later-stage overrides, exact reentry
replacement contexts, prior-handoff-only consumption, sixteen-file static context ceilings and unchanged
P00/W0 launch authority.

## P00 draft and issuance control plane

### `validate-p00-ticket-drafts.ps1`

```powershell
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
```

Checks:

- exactly three non-claimable P00 ticket drafts and three unmaterialized context drafts;
- exact package/launch class/write scope/line budgets for contracts, domain and ports;
- unresolved writer/reviewer/base/ticket/context identities;
- domain and ports remain conditional on an accepted `search-contracts` handoff;
- exact context source files, registry selectors and accepted-handoff slots;
- no architecture master, dependency implementation source or forbidden control record;
- all issued-record roots remain zero-state;
- orchestration schema v5 and launch-state P00/W0 parity;
- every workflow remains manual-only, read-only and credential-free.

A PASS proves only that drafts remain bounded and non-claimable.

### `validate-ticket-issuance-contracts.ps1`

```powershell
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1
pwsh -NoProfile -File tools/validate-ticket-issuance-contracts.ps1 -Json
```

Checks:

- `types-v1.toml` schema v2 contains exactly 47 unique, resolvable types with no alias cycle;
- exact closed `ClosedReasonCode`, `LeaseEventReasonCode`, `SupersessionReasonCode` and
  `ConsumerActionCode` registries;
- all eight record schemas are registered once and use exact canonical layouts;
- every field kind resolves and every array path uses an element type rather than a list-of-list;
- canonical top-level group order and contiguous field order;
- generic `ClosedEnum` fields carry exact equality/allowed-set rules;
- embedded `signature.record_sha256` retains signed-payload semantics while complete-file SHA-256
  remains external;
- orchestration lease/acceptance field sets exactly match `writer_lease_v1` and `package_handoff_v1`;
- package handoff paths use `handoff_id`, not API digest identity;
- documentation, orchestration and launch-state layouts/reason bindings agree;
- no legacy hyphenated control-record placeholders remain in normative issuance docs;
- all workflows are manual-only/read-only/credential-free;
- issued record directories remain zero-state.

A PASS is schema closure only. It is not context materialization, ticket/lease issuance, package acceptance,
G0/W0 evidence or runtime qualification.

## Implementation and qualification packets

### `validate-implementation-packets.ps1`

```powershell
pwsh -NoProfile -File tools/validate-implementation-packets.ps1
pwsh -NoProfile -File tools/validate-implementation-packets.ps1 -Json
```

Checks package function links, configuration ownership/example parity, `search-config` dependencies,
secret/autoupgrade floors and W3 Qdrant packets.

### `validate-current-packets.ps1`

```powershell
pwsh -NoProfile -File tools/validate-current-packets.ps1
pwsh -NoProfile -File tools/validate-current-packets.ps1 -Json
```

Checks W4 function/qualification registration, W5 function links, launch qualification-path parity,
locked currentness/unsaved/no-execute baseline flags and mandatory W5 probes.

### `validate-w5-current.ps1`

```powershell
pwsh -NoProfile -File tools/validate-w5-current.ps1
pwsh -NoProfile -File tools/validate-w5-current.ps1 -Json
```

Checks the W5 cross-contract, owner packets, finite settings, currentness/overlay probes, unselected Rust
parser probes, exact G3 evidence partition and package-local write scopes.

### `validate-proof-packets.ps1`

```powershell
pwsh -NoProfile -File tools/validate-proof-packets.ps1
pwsh -NoProfile -File tools/validate-proof-packets.ps1 -Json
```

Checks resolver/comparator/exact links, ambiguity/non-normative/frozen-denominator rules, unselected
regex/structural profiles, mandatory probes and G3 evidence.

### `validate-w7-lifecycle.ps1`

```powershell
pwsh -NoProfile -File tools/validate-w7-lifecycle.ps1
pwsh -NoProfile -File tools/validate-w7-lifecycle.ps1 -Json
```

Checks restrictive security, retention, mark/sweep, purge, restore, handles/continuations, publication,
reclaim and lifecycle receipt separation. Evidence remains unexecuted.

### `validate-w8-client-edge.ps1`

```powershell
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1
pwsh -NoProfile -File tools/validate-w8-client-edge.ps1 -Json
```

Checks generic-edge ownership, recipe closure, authority boundaries, locked settings, standalone client
contract, probe states, G4 mapping and blocked/unqualified status.

### `validate-w9-product-pulse.ps1`

```powershell
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1
pwsh -NoProfile -File tools/validate-w9-product-pulse.ps1 -Json
```

Checks Product Pulse roles, corpus cases, metrics, targets, mandatory probes, G5 mapping, locked
fairness/privacy/verdict settings and unchanged P00/W0 authority.

### `validate-w10-optional-depth.ps1`

```powershell
pwsh -NoProfile -File tools/validate-w10-optional-depth.ps1
pwsh -NoProfile -File tools/validate-w10-optional-depth.ps1 -Json
```

Checks unselected candidate profiles, package/integration/evaluation ownership packets, model/document
workers, daemon/scale/evaluation contracts, disabled probe templates, G6 maps, locked
content/migration/removal settings and unchanged P00/W0 authority.

## Evidence boundary

Passing any structural validator is not Rust compilation, runtime behavior, Windows security, Qdrant,
current-workspace, parser, exact-proof, Product Pulse, provider, optional-depth, package, gate or wave
evidence. Unavailable execution checks remain explicitly `UNAVAILABLE`; they are never inferred from a
schema or workflow PASS.
