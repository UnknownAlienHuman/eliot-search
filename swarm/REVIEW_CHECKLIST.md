# Review checklist

## Package reviewer

- Changes stay inside assigned package scope.
- Mutable state has one owner; no duplicate store, policy engine, handle table or deletion path appears.
- Shared records come from the accepted `search-contracts` digest.
- Shared traits come from the accepted `search-ports` digest; no local substitute port appears.
- Pure rules are not duplicated around `search-domain`.
- Public API contains no vendor/native type, credential, raw store locator, raw point ID or reusable
  authorization decision.
- Direct dependencies match `swarm/crates.toml` and accepted handoffs.
- Every blocking operation defines deadline/cancellation/bounds; every mutation defines idempotency.
- Failures are typed and explicitly mapped to public/protocol reasons where applicable.
- No placeholder success, fake receipt, silent fallback, `todo!()` or hidden unbounded queue exists.
- Split review occurs before 8,500 total hand-written lines; 10,000 is a hard stop.
- Raw commands and unavailable checks are reported honestly.

## Integration reviewer

- Cargo members, package directories, registry entries and assignments are identical.
- Cargo manifest dependencies match registry dependencies.
- Graph is acyclic and wave-monotonic except the declared daemon composition exception.
- Only daemon composition constructs concrete redb, OS-secret, Qdrant process/data-plane adapters.
- Contracts, domain and ports publish immutable accepted API digests before consumers start.
- Access/currentness happen before retrieval/scoring.
- Source readback, publication, handles, ordinary reclaim and purge remain distinct end-to-end invariants.
- Optional providers remain blocked, removable and separately qualified.
- Wave evidence is complete; compilation alone is not acceptance.
