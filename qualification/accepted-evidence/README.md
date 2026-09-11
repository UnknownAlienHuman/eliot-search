# Accepted evidence digest qualification

Run:

```powershell
cargo test --locked -p xtask --test accepted_evidence_parity
pwsh -NoProfile -File tools/validate-accepted-evidence-digest.ps1
```

The Rust parity suite consumes the frozen byte vectors under
`fixtures/tooling/accepted-evidence/` and the ten-case qualification inventory.
It covers deterministic ordering, empty evidence, order sensitivity, duplicate
requirements, unknown fields, invalid digests, artifact mismatch, invalid
identifiers, bounds and null/float rejection.

Python is not required. Passing proves only semantic digest-profile conformance.
