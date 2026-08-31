# W3 agent draft qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-w3-agent-drafts.ps1 -Json
```

The validator checks the nine W3 package packets, eighteen non-claimable drafts, dependency groups, context ceilings, package-local write scopes, daemon replacement context, current P00/W0 launch state, Qdrant qualification non-success state and manual workflow policy.

A PASS is structural only. It does not accept G1 or W2_G1, qualify/select/download Qdrant, materialize context, issue a ticket/lease, authorize implementation, enable indexed mode, accept a package or emit a W3/G2 receipt.
