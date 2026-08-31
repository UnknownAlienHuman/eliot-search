# W3 milestone packet qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-w3-milestone-packets.ps1 -Json
```

A PASS proves only bounded packet topology: nine packages, four ordered checkpoints per package, package-only scopes, exact dependency handoffs, G1/W2_G1/Qdrant qualification blocking, disabled indexed mode and manual workflow policy.

It does not authorize a writer, qualify Qdrant, accept any implementation or emit a W3/G2 receipt.
