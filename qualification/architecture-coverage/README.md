# Architecture ownership coverage qualification

Run:

```powershell
pwsh -NoProfile -File tools/validate-architecture-coverage.ps1 -Json
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
```

The first validator closes the static ownership graph from Architecture 8.4 Part I to:

```text
45 packages and assignments
42 package-local function sources plus 3 foundation sources
source-derived package-qualified operations
45 package module packets / 479 logical modules
S0-S39 architecture sections
C00-C30 capability cells
INV-01..INV-30 invariants
23 shared ports and exact methods
20 configuration sections
217 unique P00 named type/schema/registry symbols
11 public recipes
51 reason and error codes
P00-P18 delivery slices
```

The second validator ensures the five named type completions are part of the manifest-closed P00 contract
pack and every bounded W0 context remains non-claimable.

A PASS proves only static ownership and source-registry closure. It does **not** prove that Rust modules or
operations are implemented, dependencies compile, Windows/Qdrant/provider behavior is qualified, a
package handoff is accepted, a gate/wave is accepted or Product Pulse passes.

The 40 cases in `cases-v1.toml` remain `UNAVAILABLE` until the exact branch/commit validators execute and
an integration reviewer records the raw outputs. Even then they are structural evidence only.
