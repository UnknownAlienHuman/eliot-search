# Context materialization plan qualification

The planner and behavioral corpus are Rust-owned.

```powershell
cargo test --locked -p xtask --test context_materialization_parity
cargo test --locked -p xtask --test context_materialization_builder
cargo run --locked -p xtask -- validate context-materialization-plan --json
```

The PowerShell entrypoints invoke the same locked Cargo tooling:

```powershell
pwsh -NoProfile -File tools/validate-context-materialization-plan.ps1 -Json
pwsh -NoProfile -File tools/plan-context-materialization.ps1 `
  -Candidate artifacts/context-artifact-candidates/<package>/<candidate>.json
```

The behavioral corpus covers:

- missing external selection;
- payload generation with both signatures absent;
- stable operation identity across signature collection;
- complete dual-signature proposal;
- partial signature blocking;
- actor conflict;
- artifact/readback mismatch;
- signature payload mismatch;
- candidate/bundle tampering;
- idempotent ordinary output and conflict rejection;
- accepted-handoff evidence projection.

The planner reads canonical local candidate/bundle/selection artifacts, renders the prospective
`context_manifest_v1` payload and writes only ignored artifacts under
`artifacts/context-materialization-plans/`. It never writes under `swarm/context-manifests/`.

Passing proves proposal/compiler conformance only. It does not store an artifact, commit a context
manifest, issue a ticket or lease, accept a package/gate/wave or create implementation authority.
