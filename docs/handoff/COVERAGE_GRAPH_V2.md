# Coverage graph v2

This is the exact machine-checked ownership graph from architecture and package contracts to Cargo
packages and package-local logical modules. It does not claim Rust implementation.

## Closed relations

- **45 Cargo packages** and **479 declared logical modules**;
- **664 package-qualified operations** mapped to exactly one reviewed module in the same package;
- **3499 Markdown heading nodes** across **397 tracked documentation files**;
- **2801 implementation/principle/qualification nodes** mapped to package modules;
- **698 governance/navigation nodes** explicitly classified as non-crate-owned rather than forced into a fake product crate;
- **206 Cargo dependency edges** mapped from a consumer module to the producer public entry;
- **30 later-wave dependency edges** bound to exact progressive stage re-entry records;
- **0 weak implementation modules** after relation aggregation.

## Route ownership policy

Operation, documentation and dependency routes in `swarm/coverage/*.toml` are reviewed machine inputs.
The Rust generator reconciles their derived counts and report; it does not guess ownership from names,
word similarity or package heuristics. New or changed routes require an explicit reviewed registry change.

## Operation routing quality

```text
{
  "package_rule": 190,
  "semantic": 474
}
```

`public_facade` and `semantic_low` routes are merge-blocking. The committed operation registry records
the exact source file, source section, selected module, routing class and score for review.

## Reconciliation and validation

```powershell
cargo run --locked --quiet -p xtask -- generate coverage-graph --check --json
cargo run --locked --quiet -p xtask -- generate package-maps --check --json
cargo run --locked --quiet -p xtask -- validate coverage-graph --json
cargo run --locked --quiet -p xtask -- validate package-maps --json
cargo run --locked --quiet -p xtask -- validate architecture-coverage --json
cargo run --locked --quiet -p xtask -- validate architecture-coverage-contracts --json
```

The validators reject missing or orphan operations, stale documentation headings, cross-package module
routes, configuration/recipe/port owner drift, missing dependency or re-entry edges, weak implementation
modules and any automatic trigger in the permanent validation workflow.

## Authority ceiling

These registries are design/ownership evidence only. They create no ticket, lease, accepted package
handoff, gate receipt, wave receipt or implementation authority. Launch state remains P00/W0.
