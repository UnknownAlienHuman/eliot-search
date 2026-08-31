# Context materialization plan qualification

Run:

```powershell
python qualification/context-materialization/test_context_materialization_plan_v1.py
pwsh -NoProfile -File tools/validate-context-materialization-plan.ps1 -Json
```

The corpus covers missing selection, payload generation, complete dual-signature proposal, partial and
mismatched signatures, actor conflict, artifact mismatch, candidate/bundle tampering, idempotent local
writes and accepted-handoff evidence projection.

Passing proves proposal/compiler conformance only. It does not store an artifact, commit a
`context_manifest_v1`, issue a ticket/lease or create implementation authority.
