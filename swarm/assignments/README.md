# Bounded package assignments

Each file in this directory is the implementation packet for exactly one Cargo package. The
orchestrator supplies one packet to one writer together with root/family/package `AGENTS.md`, `../ASSIGNMENT_PROTOCOL.md` and accepted public dependency handoffs.

Assignments are integration-owned and immutable for a writer's task. A package writer must not edit
`swarm/assignments/` to make implementation easier. Missing or contradictory load-bearing semantics use
`../CONTRACT_CHANGE_TEMPLATE.md`.

A packet is intentionally small compared with the architecture master and contains:

- mission, ownership and forbidden ownership;
- logical primitives and operations;
- required invariants and typed reason codes;
- suggested internal module plan;
- mandatory tests/evidence;
- dependency and line-budget rules;
- launch gate and architecture traceability.

The file name is the exact Cargo package name: `swarm/assignments/<package>.md`.
`../launch-state.toml` decides whether the packet may currently be launched.
