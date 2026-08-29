# Development tools

These utilities are never linked into production binaries.

## Swarm structure validation

From the repository root on Windows or PowerShell 7:

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1
```

Machine-readable output:

```powershell
pwsh -NoProfile -File tools/validate-swarm.ps1 -Json
```

The validator checks:

- Cargo members, registry packages, directories and assignments;
- registry/Cargo internal dependency equality;
- unknown dependencies, cycles and wave monotonicity;
- daemon progressive-composition exception;
- package and launch-state counts;
- line limits and forbidden placeholder macros;
- required P00 contract-pack files.

It uses only built-in PowerShell/.NET APIs and creates no production dependency. CI activation remains
an integration-owner decision after the relevant launch gate.
