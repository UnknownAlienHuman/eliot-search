# Coverage graph v2

This is the exact machine-checked ownership graph from architecture and package contracts to Cargo
packages and package-local logical modules. It does not claim Rust implementation.

## Closed relations

- **45 Cargo packages** and **479 declared logical modules**;
- **664 package-qualified operations** mapped to exactly one module in the same package;
- **3499 Markdown heading nodes** across **397 tracked documentation files**;
- **2801 implementation/principle/qualification nodes** mapped to package modules;
- **698 governance/navigation nodes** explicitly classified as non-crate-owned rather than forced into a fake product crate;
- **206 Cargo dependency edges** mapped from a consumer module to the producer public entry;
- all 20 configuration sections bound to an owner module;
- all 11 recipes bound to one execution module per primary execution package;
- **0 weak implementation modules** after relation aggregation.

## Operation routing quality

```text
{
  "package_rule": 190,
  "semantic": 474
}
```

`public_facade` is permitted only when the documented operation itself is the package entry/facade
operation. The committed operation registry records the exact source file, source section, selected
module, routing class and score for review.

## Validation

```powershell
python tools/generate-coverage-graph-v2.py --check
python tools/validate-coverage-graph-v2.py --json
python tools/validate-architecture-coverage.py --json
```

The validators reject missing or orphan operations, stale documentation headings, cross-package module
routes, configuration/recipe owner drift, missing dependency edges, weak implementation modules and any
change that reintroduces an automatic permanent workflow trigger.

## Authority ceiling

These registries are design/ownership evidence only. They create no ticket, lease, accepted package
handoff, gate receipt, wave receipt or implementation authority. Launch state remains P00/W0.
