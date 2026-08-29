# Review checklist

## Package reviewer

- Changes stay inside the package write scope.
- The implementation owns only responsibilities listed in package `AGENTS.md`.
- Forbidden responsibilities and vendor types did not leak into the public surface.
- Direct dependencies match `swarm/crates.toml`; new dependencies have an accepted boundary review.
- No duplicate local contract was invented to bypass `search-contracts`.
- Failure paths emit typed reasons and never manufacture completeness/currentness/success.
- Invariant, negative, property and fault tests cover the package-owned failure states.
- No `todo!()`, placeholder success, silent fallback, fake receipt or hidden unbounded queue exists.
- Hand-written Rust remains below 10,000 lines or has an accepted split decision.
- Raw command outcomes and unavailable checks are reported honestly.

## Integration reviewer

- Dependency graph remains acyclic and respects contract→capability→adapter→runtime direction.
- Root lockfile/toolchain/CI/generated changes are made only by the integration owner.
- Downstream packages consume immutable accepted dependency commits.
- Wire/schema changes are versioned and generated projections match the contract source.
- Access/currentness restrictions happen before retrieval/scoring, not only after.
- Source-backed readback, publication and purge invariants remain end to end.
- Optional providers remain gated, disabled and removable.
- Wave exit evidence is complete; compilation alone is not treated as a gate or product verdict.
