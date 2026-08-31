# W4 agent draft qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-w4-agent-drafts.ps1 -Json
```

The validator checks the nine W4 package packets, eighteen non-claimable drafts, exact dependency groups, context ceilings, package-local write scopes, daemon replacement context, current P00/W0 launch state, unexecuted query baseline/probes and manual workflow policy.

A PASS is structural only. It does not accept G1 or W3, materialize context, issue a ticket/lease, authorize implementation, enable query serving, accept a package or emit a W4/G2 receipt.
