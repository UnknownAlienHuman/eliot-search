# Development tools

These utilities are never linked into production binaries.

## Swarm structure validation

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

The validator checks Cargo/registry/package/assignment identity, exact internal dependencies,
cycles/waves, launch state, line limits and the P00 manifest. JSON output includes SHA-256 for the P00
manifest and every required contract-pack file, suitable for the future W0 receipt.

It uses built-in PowerShell/.NET APIs and creates no production dependency. The structural workflow is
not runtime, security, Qdrant or product-acceptance evidence.
