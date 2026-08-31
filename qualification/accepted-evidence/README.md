# Accepted evidence digest qualification

Run:

```powershell
python qualification/accepted-evidence/test_accepted_evidence_digest_v1.py
pwsh -NoProfile -File tools/validate-accepted-evidence-digest.ps1
```

The corpus covers deterministic ordering, empty evidence, order sensitivity, duplicate requirements,
unknown fields, invalid digests, artifact mismatch, invalid identifiers, bounds and null/float rejection.
Passing proves only semantic digest-profile conformance.
