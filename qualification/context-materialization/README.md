# Context materialization plan qualification

Run the existing twelve-case planner corpus, then the Rust structural validator:

```powershell
python qualification/context-materialization/test_context_materialization_plan_v1.py
cargo run --locked -p xtask -- validate context-materialization-plan --json
```

The PowerShell compatibility entrypoint invokes the same Rust validation command:

```powershell
pwsh -NoProfile -File tools/validate-context-materialization-plan.ps1 -Json
```

The corpus covers missing selection, payload generation, complete dual-signature proposal, partial and mismatched signatures, actor conflict, artifact mismatch, candidate/bundle tampering, idempotent local writes and accepted-handoff evidence projection.

The structural validator itself no longer requires Python. The planner and its twelve-case behavioral corpus remain Python-owned until their complete candidate assembly, rendering and write semantics are ported; they must not be deleted piecemeal.

Passing proves proposal/compiler conformance only. It does not store an artifact, commit a `context_manifest_v1`, issue a ticket/lease or create implementation authority.
