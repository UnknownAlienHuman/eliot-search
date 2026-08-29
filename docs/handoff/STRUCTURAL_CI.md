# Structural CI scope

`Swarm structure` is a narrow scaffold-integrity workflow. It becomes meaningful now that the P00
contract pack, package registry and shared port owner exist.

It checks:

- Cargo members and package manifests against `swarm/crates.toml`;
- package directories and one assignment per registry package;
- dependency existence, cycles and first-wave monotonicity;
- launch-state counts and authorization;
- line limits and forbidden placeholder macros;
- presence of the exact P00 contract-pack files.

It does **not** prove:

- Rust compilation or contract correctness;
- Windows process/ACL/secret behavior beyond running the validator on a Windows runner;
- Qdrant/redb/CAS fault behavior;
- security noninterference;
- latency, resource budgets or Product Pulse acceptance.

The workflow uses read-only repository permissions and a commit-pinned checkout action. Passing it is a
merge prerequisite for scaffold changes, not a W0 receipt or product gate.

Current pinned checkout identity:

```text
actions/checkout v7.0.1
commit 3d3c42e5aac5ba805825da76410c181273ba90b1
```

Updating the action requires official release verification and an explicit commit-SHA change.
