# Assignment packet schema

Every `swarm/assignments/<package>.md` is interpreted with `ASSIGNMENT_PROTOCOL.md` and must include the following package-specific sections. The integration reviewer
rejects a packet that omits one.

1. **Identity and launch gate** — package path, capability cell, wave/P stage, explicit gate.
2. **Mission** — one causal responsibility.
3. **Read set** — exact bounded inputs; future-wave or dependency internals are excluded.
4. **Write boundary** — exactly one package directory.
5. **Owned responsibility / non-ownership** — positive and negative authority.
6. **Logical primitives** — records, state, receipts and opaque identities owned by the package.
7. **Logical operations** — behavior-level signatures with typed failure semantics.
8. **Required invariants** — safety/currentness/access/determinism claims the package must prove.
9. **Failure surface** — public reason codes; no silent fallback or empty-success substitution.
10. **Internal modules** — a suggested file plan, explicitly not extra crates.
11. **Contract-first order** — tests and ports before implementation.
12. **Exit evidence** — deterministic, fault, security or qualification fixtures.
13. **Dependency rules** — accepted public ports only; vendor leakage prohibited.
14. **Size guard** — ordinary target, split review and 10,000-line hard stop.
15. **Handoff** — exact receipt expected by downstream agents.

## Semantic requirements

- Names in a packet are logical abstractions, not mandatory Rust spelling.
- A packet cannot authorize a behavior forbidden by Architecture 8.4.
- A packet may narrow a package's scope, but cannot broaden its architecture ownership.
- A later-wave hardening packet reuses the same package; it does not create a second owner.
- Optional packages state the exact acceptance/ADR gate and remain disabled by default.
- Composition packages receive dependency handoffs by active feature/wave only; they do not read every
  package mentioned in the final topology.
