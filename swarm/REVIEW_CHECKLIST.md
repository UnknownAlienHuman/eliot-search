# Review checklist

## Package reviewer

- Changes stay inside the package write scope.
- The implementation owns only responsibilities listed in package `AGENTS.md` and assignment.
- Mutable state has one owner; no duplicate store, policy engine, handle table or deletion path appeared.
- Public ports match `docs/handoff/PORT_CATALOG.md` and contain no vendor types, credentials, raw store
  locators, raw point IDs or reusable authorization decisions.
- Direct dependencies match `swarm/crates.toml`; new dependencies have an accepted boundary review.
- No duplicate local contract was invented to bypass `search-contracts` or `search-domain`.
- Failure paths emit typed reasons and never manufacture completeness, currentness or success.
- Invariant, negative, property and fault tests cover the package-owned failure states.
- No `todo!()`, placeholder success, silent fallback, fake receipt or hidden unbounded queue exists.
- Hand-written Rust remains below 10,000 lines and split review occurred before 8,500 when applicable.
- Raw command outcomes and unavailable checks are reported honestly.

## Integration reviewer

- Cargo members, registry entries, assignments and package directories are identical.
- Dependency graph remains acyclic and respects contract → capability → adapter → daemon composition.
- Only `eliot-searchd` constructs concrete redb, OS-secret, Qdrant process and Qdrant data-plane adapters.
- Query/lifecycle crates consume ports and do not directly open concrete stores.
- Root lockfile/toolchain/CI/generated changes are made only by the integration owner.
- Downstream packages consume immutable accepted dependency commits and API/port digests.
- Access/currentness restrictions happen before retrieval/scoring, not only after.
- Source-backed readback, publication, handle expansion, ordinary reclaim and security purge remain
  separate end-to-end invariants.
- Optional providers remain gated, disabled and removable.
- Wave exit evidence is complete; compilation alone is not treated as a gate or product verdict.
