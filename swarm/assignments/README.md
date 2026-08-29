# Bounded package assignments

Each file is the implementation packet for one Cargo package. `swarm/crates.toml` is authoritative for
package path, exact direct dependencies, assignment path, earliest wave, optionality and size target.

The orchestrator supplies:

- root/family/package `AGENTS.md`;
- `ASSIGNMENT_PROTOCOL.md`;
- exactly one assignment;
- the relevant `docs/contracts/p00/` files for W0 only;
- accepted API/port handoffs for every dependency listed in the registry;
- immutable assignment issue and base commit.

Assignment prose defines mission, owned/forbidden state, logical operations, invariants, failures,
tests and internal module suggestions. A header that omits a shared foundation dependency does not
override `swarm/crates.toml`; the registry must be checked before launch.

Writers cannot edit assignments. Missing or contradictory load-bearing semantics use
`CONTRACT_CHANGE_TEMPLATE.md`. `launch-state.toml` decides whether an assignment may currently run.
