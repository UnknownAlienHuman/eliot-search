# Architecture 8.4 package-coverage audit

**Audited base:** `73ab0c415960ec1322f4b367a2325ce7916301b0`  
**Architecture Part I SHA-256:** `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`  
**Conclusion:** package topology existed, but exhaustive function/module/schema/task ownership was not
machine-closed. This change closes static ownership only; implementation remains absent.

## Findings before correction

The repository already had:

```text
45 Cargo packages
3 foundation contract sources
42 package-local FUNCTIONS.md files
20 configuration sections
stage and qualification packets through optional depth
```

That proved package presence, not exhaustive architectural closure. The following load-bearing gaps
remained:

1. No machine registry mapped all 45 packages to internal logical modules.
2. No validator derived every package-qualified operation from all 42 `FUNCTIONS.md` sources.
3. S0–S39, C00–C30 and INV-01–INV-30 had no exact package/module crosswalk.
4. The 23 shared ports had no machine-checked method-to-implementation-owner registry.
5. `ResidencyPolicyPort` had a floating “selected implementation” owner instead of one crate.
6. `ClockPort` had a floating “daemon/platform adapter” owner instead of one exact private module.
7. P00 schemas were not exhaustively enumerated with separate shape, meaning and mutable-state owners.
8. `TYPE_REGISTRY.md` contained 115 named symbols, not the previously assumed 50.
9. Five types used by field-level schemas were not explicitly defined:
   `RecipeIdV1`, `RecipeBodyV1`, `ComparisonAxis`, `ProtocolRange`, `PackageOpaque`.
10. The eleven recipes and P00–P18 delivery tasks had no machine ownership closure.
11. Existing readiness/ownership audits could pass while a specific schema, method, section, invariant or
    delivery obligation remained unowned.

## Corrections

### Module closure

`swarm/module-packets.toml` and `swarm/modules/*.toml` now define:

```text
45 package packets
479 package-local logical modules
one public entry module per package
maximum 15 modules per package
all public operations entering through that entry
all mutable package state confined to declared package modules
```

This is a logical implementation layout, not generated Rust files. Package writers may refine private
spelling only if operation ownership, state ownership, line limits and public handoffs remain unchanged.

### Operation closure

`swarm/coverage/operations.toml` makes every operation identity package-qualified:

```text
<package>::<operation>
```

The validator derives operations from all registered `FUNCTIONS.md` code headings/signatures, rejects an
unregistered/orphan function file, requires at least one operation per non-foundation package and binds
all operations to the package public entry module.

### Architecture closure

Machine registries now cover:

```text
S0-S39       40 normative sections
C00-C30      31 capability cells
INV-01..30   30 non-negotiable invariants
P00-P18      19 delivery slices
```

Every entry names concrete packages and valid package-local modules. Every one of the 45 packages appears
in at least one delivery slice with required outputs and exit evidence.

### Port closure

`swarm/coverage/ports.toml` freezes all 23 shared ports, exact method inventories and one implementation
owner/module each. In particular:

```text
ClockPort             → private eliot-searchd::adapters platform adapter
ResidencyPolicyPort   → search-revision-store::residency
```

`search-ports` remains the sole trait owner. Concrete vendor/platform types remain private.

### Type and schema closure

The P00 public contract surface now contains:

```text
115 named symbols already present in TYPE_REGISTRY.md
  5 named type completions
 97 additional support/record/result/protocol/reason schemas
---
217 unique named type/schema/registry symbols
```

The five completions are defined in `docs/contracts/p00/TYPE_COMPLETIONS.md`, included in the P00 manifest
and bounded W0 contexts, and recorded as resolved challenge PC-018.

For every symbol, the coverage packets name:

```text
one shape owner package/module
one pure meaning owner package/module or NONE
one mutable state owner package/module or NONE
optional secondary state consumers
exact source document(s)
```

### Recipe, reason and task closure

The registries close:

```text
11 RecipeIdV1 values and result/execution owners
31 SearchReasonCodeV1 values
10 ProtocolErrorCode values
10 ContractErrorCode values
45 package assignment tasks
19 architecture delivery tasks
```

## Validator

Run:

```powershell
pwsh -NoProfile -File tools/validate-architecture-coverage.ps1 -Json
pwsh -NoProfile -File tools/validate-p00-ticket-drafts.ps1 -Json
```

The first validator derives architecture cells/sections/invariants/delivery IDs and port methods directly
from normative source documents, then checks all registry/package/module references. It also checks
source-derived operations, assignments, configuration owners, P00 named types, top-level YAML schema
labels, recipes and reasons.

The second validator is now manifest-driven and verifies the 13-file P00 contract pack plus its bounded,
non-claimable W0 contexts.

## Honest remaining state

```text
static ownership coverage:              closed by registry, pending executed validator receipt
Rust package modules:                   not implemented
package operations:                     not implemented
accepted package handoffs:              0
accepted gates/wave receipts:           0
Windows/Qdrant/provider qualification:  not executed
Product Pulse:                          unavailable
launch authority:                       P00 / W0 / search-contracts only
```

A green structural validator does not prove implementation correctness. Actual closure requires each
package implementation, package-local and integration tests, independent review, immutable package/API
handoffs and the separate gate/wave evidence defined by Architecture Part I.
