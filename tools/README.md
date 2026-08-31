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
launch state, line limits and the P00 contract manifest. At P00/W0 it also requires exactly
`search-contracts` authorized and exactly `search-domain` plus `search-ports` conditional.

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
replacement contexts, prior-handoff-only consumption, sixteen-file later-stage static context ceilings
and unchanged P00/W0 launch authority.

## P00 draft and issuance control plane

### `validate-p00-ticket-drafts.ps1`

```powershell
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
```

Checks:

- exactly three non-claimable schema-v2 P00 ticket drafts and three schema-v2 unmaterialized context
  drafts;
- ticket drafts contain no lease identity and keep signed-payload and complete-file digest slots separate;
- context drafts keep manifest-record and writer-artifact refs/digests separate;
- exact package/launch class/write scope/line budgets for contracts, domain and ports;
- unresolved writer/reviewer/base/ticket/context identities;
- domain and ports remain conditional on an accepted `search-contracts` handoff;
- exact context source files, registry selectors, accepted-handoff slots and unavailable-check order;
- ordinary P00 contexts remain at or below sixteen source files;
- the sole `search-contracts` exception remains at or below twenty-four sources and equals the exact
  manifest-closed P00 contract pack plus its fixed integration instructions in canonical order;
- every context remains within six registry fragments, one accepted-handoff slot and exactly one
  writer-visible artifact;
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
- every consumed control-record digest uses an explicit `exact_record_file_sha256` field name and rejects
  ambiguous `ticket.sha256`, `lease.sha256`, `submission.sha256`, `review.sha256` and equivalent paths;
- every signed record carries at least one `ImmutableSignatureRef` bound to its signed-payload digest;
- `context_manifest_v1` carries distinct materializer and reviewer signature refs;
- orchestration lease/acceptance field sets exactly match `writer_lease_v1` and `package_handoff_v1`;
- rejected work returns to `READY` through a new context/ticket revision and no active lease, rather than
  bypassing the separate `READY → LEASED` transition;
- package handoff paths use `handoff_id`, not API digest identity;
- documentation, orchestration and launch-state layouts/reason bindings agree;
- no legacy hyphenated control-record placeholders remain in normative issuance docs;
- all workflows are manual-only/read-only/credential-free;
- issued record directories remain zero-state.

A PASS is schema closure only. It is not context materialization, ticket/lease issuance, package acceptance,
G0/W0 evidence or runtime qualification.

### `validate-p00-foundation-acceptance.ps1`

```powershell
pwsh -NoProfile -File tools/validate-p00-foundation-acceptance.ps1
pwsh -NoProfile -File tools/validate-p00-foundation-acceptance.ps1 -Json
```

The PowerShell wrapper invokes the standard-library Python validator
`tools/validate-p00-foundation-acceptance.py`. It checks:

- exact topological package order: contracts, then conditional domain/ports;
- complete context → ticket → lease → acknowledgement → submission → independent review → handoff
  ladder for each package;
- exact package-only scopes and distinct conditional predecessor/parallelism declarations;
- exact P00-A through P00-D checkpoint sequence;
- equality between the ten machine acceptance evidence rows and the exact G0 evidence registry;
- raw-output and independent-review requirements for every G0 evidence item;
- exact W0 package set and W1 `G0` plus `W0` prerequisites;
- current P00/W0 launch classification and zero-state counters;
- non-claimable schema-v2 ticket/context drafts and exact conditional handoff slots;
- package/function/stage/gate/orchestration/launch registry parity;
- zero issued records under all protected roots;
- matrix/navigation closure and manual-only/read-only/credential-free workflows.

Its JSON output keeps all package, G0, W0 and W1 authority claims false. A PASS proves only that the
acceptance boundary is structurally closed; it does not create any record or authorize implementation.

### Ticket issuance advisory planner v2

```powershell
pwsh -NoProfile -File tools/plan-ticket-issuance.ps1 `
  -Package search-contracts `
  -Output artifacts/ticket-issuance-plans/search-contracts.json

pwsh -NoProfile -File tools/validate-ticket-issuance-plan.ps1 -Json
```

The dependency-free planner reads one immutable Git tree, validates schema-v2 P00 drafts,
manifest-owned 16/24 source ceilings, exact selectors, accepted prerequisite handoffs, current-package
control conflicts and workflow policy, then emits a deterministic advisory JSON decision. Output is
limited to stdout or ignored files under `artifacts/ticket-issuance-plans/`.

`READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW` authorizes nothing. The planner never materializes context,
issues a ticket or lease, publishes a handoff, accepts G0/W0 or advances launch state. The validator runs
the 30-case corpus and requires the current repository decision to remain `BLOCKED_MISSING_SELECTION`.

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
