# Historical W7 access hardening

The previous W7 programme attempted to harden a grant/snapshot/checkpoint framework which is not connected
to product query serving. Do not implement stronger hashes, owner-issued permits, more generation axes or
another access registry around it.

Current baseline:

```text
PublishedViewGuard
→ non-widening ResolvedScope
→ one Qdrant/base-IDF population
→ pinned manifest and retained evidence
```

Owning-account local use needs no grant. A selected restricted-client adapter owns its concrete session,
allowed subset, limits, revocation and windows. It cancels the session directly; it does not issue portable
clean/permit records to planner, executor, validator, projector and continuation packages.

Remove `ValidatedGrant`, live phase checkpoints, eligibility/predicate digests, overlap proofs, per-member
safe legs, `LiveExpansionBarrier` and the durable security-mutation barrier with their consumers. Preserve
only explicit non-widening selection and real restricted-session checks documented in [FUNCTIONS.md](FUNCTIONS.md).
