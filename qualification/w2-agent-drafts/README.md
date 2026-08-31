# W2 agent draft qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-w2-agent-drafts.ps1 -Json
```

The validator checks the eight W2 package packets, sixteen non-claimable drafts, exact A/B/C dependency
order, daemon W2 re-entry replacement semantics, bounded contexts, package-local write scopes, current
P00/W0 launch authority and manual workflow policy.

A PASS is structural only. It does not accept G0/W1, materialize context, issue a ticket/lease, authorize
implementation, accept a package or emit the W2/G1 receipt.
