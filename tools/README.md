# Development and structural-validation tools

These utilities are repository-development tools only. They are never linked
into production binaries, do not issue control records and do not create
package, gate or wave acceptance.

Required repository tooling has no Python or Node runtime dependency. Rust
validators, generators and advisory builders are owned by the locked `xtask`
package. PowerShell files under `tools/` are bounded compatibility wrappers or
repository-local structural checks. Every workflow under `.github/workflows/`
is `workflow_dispatch`-only, declares `contents: read` and disables checkout
credential persistence.

## Rust command surface

Run commands from the repository root:

```powershell
cargo run --locked -p xtask -- <command>
```

The registered command surface is:

```text
validate accepted-evidence-digest
compute accepted-evidence-digest <record> [--json-array]

build context-artifact-candidate --package <name> --base-commit <algorithm:oid> ...
build context-materialization-plan --candidate <path> ...
build ticket-issuance-plan --package <name> ...

generate coverage-graph [--check] [--json]
generate package-maps [--check] [--json]

validate p00-ticket-drafts [--json]
validate w1-agent-drafts [--json]
validate w2-agent-drafts [--json]
validate w3-agent-drafts [--json]
validate w4-agent-drafts [--json]
validate w1-milestone-packets [--json]
validate w2-milestone-packets [--json]
validate w3-milestone-packets [--json]
validate integration-bootstrap [--root <path>] [--allow-missing-lock] [--json]
validate architecture-coverage [--json]
validate architecture-coverage-contracts [--json]
validate coverage-graph [--json]
validate package-maps [--json]
validate context-artifact-candidate [--root <path>] [--json]
validate context-materialization-plan [--json]
validate ticket-issuance-plan [--root <path>] [--json]
validate implementation-program [--json]
validate p00-foundation-acceptance [--json]
validate qdrant-boundary [--json]
```

`xtask` prints the canonical usage text and exits `2` for an unregistered
command. Generators support only their declared check/output modes; validators
remain read-only.

## PowerShell entrypoints

Compatibility wrappers preserve the existing operator-facing command names and
forward to the locked Rust owner. Examples:

```powershell
pwsh -NoProfile -File tools/validate-p00-foundation-acceptance.ps1 -Json
pwsh -NoProfile -File tools/validate-architecture-coverage.ps1 -Json
pwsh -NoProfile -File tools/validate-coverage-graph-v2.ps1 -Json
pwsh -NoProfile -File tools/validate-package-maps-v2.ps1 -Json
pwsh -NoProfile -File tools/validate-context-artifact-candidate.ps1 -Json
pwsh -NoProfile -File tools/validate-ticket-issuance-plan.ps1 -Json
pwsh -NoProfile -File tools/validate-rust-only-entrypoints.ps1
```

The remaining bounded PowerShell structural checks cover repository topology,
function packets, stage read sets and the later-wave packet registries. They may
inspect checked-in files and invoke locked Cargo commands, but may not launch a
second product runtime, issue authority records, infer unavailable evidence or
restore a Python/Node validator.

## Main validation families

### Topology and ownership

- `validate-swarm.ps1` checks Cargo/registry/package/assignment identity,
  dependency parity, graph cycles/waves, launch state and line limits.
- `validate-function-packets.ps1` checks package/function/write-scope parity and
  operation-contract structure.
- `validate-stage-readsets.ps1` checks W0-W10 composition, later-stage context
  replacement and accepted-handoff-only consumption.
- `validate-architecture-coverage.ps1`, `validate-coverage-graph-v2.ps1` and
  `validate-package-maps-v2.ps1` execute the Rust-owned architecture and package
  closure gates.

### Draft, context and ticket control

- `validate-p00-ticket-drafts.ps1` verifies bounded non-claimable ticket/context
  drafts and zero issued-record state.
- `validate-ticket-issuance-contracts.ps1` checks closed schemas, signatures,
  reason registries and orchestration parity.
- `build-context-artifact-candidate.ps1`,
  `plan-context-materialization.ps1` and `plan-ticket-issuance.ps1` produce only
  bounded advisory artifacts in their declared output roots.
- `validate-context-artifact-candidate.ps1`,
  `validate-context-materialization-plan.ps1` and
  `validate-ticket-issuance-plan.ps1` validate those artifacts without issuing
  tickets, leases or handoffs.
- `validate-p00-foundation-acceptance.ps1` executes the Rust-owned P00 acceptance
  boundary. Its JSON result keeps package, G0, W0 and W1 authority claims false.

### Implementation and later waves

- `validate-implementation-packets.ps1` and
  `validate-current-packets.ps1` check registered implementation and
  qualification packet links.
- `validate-w5-current.ps1` checks currentness, overlay and parser qualification
  declarations.
- `validate-proof-packets.ps1` checks resolver/comparator/exact-proof packet
  closure.
- `validate-w7-lifecycle.ps1` checks lifecycle, restrictive security,
  retention/purge/restore and receipt separation.
- `validate-w8-client-edge.ps1`, `validate-w9-product-pulse.ps1` and
  `validate-w10-optional-depth.ps1` check their respective optional or
  later-wave boundaries without enabling them.

## Runtime-boundary regression

Run both T41 regression gates:

```powershell
cargo test --locked -p xtask --test tooling_runtime_boundary --test tooling_runtime_inventory
```

`tooling_runtime_boundary` freezes explicit retired-validator-to-`xtask`
ownership mappings. `tooling_runtime_inventory` walks every executable file
under `tools/` and `.github/workflows/`, rejects Python/Node-family source files,
runtime manifests and executable commands, and fails closed on symbolic links.

## Evidence boundary

A structural PASS is not Rust compilation, runtime behavior, native Windows
security, live Qdrant qualification, current-workspace truth, package acceptance
or a gate/wave receipt. Unavailable execution remains `UNAVAILABLE`; it is never
inferred from source presence, a schema, a wrapper or a workflow definition.
