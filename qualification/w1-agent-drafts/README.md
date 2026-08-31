# W1 agent draft qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-w1-agent-drafts.ps1 -Json
```

The validator checks the seven package packets, fourteen non-claimable drafts, exact dependency groups,
context ceilings, package-local write scopes, current P00/W0 launch state and manual workflow policy.

A PASS is structural only. It does not accept G0/W0, materialize context, issue a ticket/lease, authorize
implementation, accept a package or emit a W1 receipt.
