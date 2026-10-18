# search-handles — implemented functions

## Current API

| Entry point | Actual behavior |
|---|---|
| `HandleStore::new(policy)` | Creates in-memory token-digest and ID maps. |
| `mint_ephemeral(target, binding, crypto, cancelled)` | Checks the supplied cancellation flag and active-record quota; obtains mint material and inserts a record. |
| `mint_durable_source(target, binding, crypto, cancelled)` | Uses the same maps for a retained-source target. No persistence or lease acquisition. |
| `resolve(handle, crypto)` | Looks up the token digest and compares handle identity/class/revision. Expansion revalidation is separate. |
| `revalidate(record, context, authorization_digest)` | Checks record state, TTL, supplied binding/source/view restrictions and requested bytes against target length. Returns a cloned target in a permit. |
| `expand(record, before, after, authorization_digest, port, cancelled_before_emission)` | Calls `HandleReadbackPort::read_exact`, compares readback fields/byte ceiling, checks cancellation after the read, then revalidates against the supplied second context. |
| `invalidate(scope, generation)`, `expire(now)` | Mark selected records terminal; do not remove either map entry. |
| `begin_security_invalidation`, `begin_binding_invalidation` | Start cursor-based invalidation with `advance` / `finish`. |

See [lib.rs](src/lib.rs) and [invalidation.rs](src/invalidation.rs). There is no `apply_live_limits`,
durable storage port, asynchronous cancellation or explicit readback deadline in this API.

## Corrections and consolidation

- The store is not bounded by its live quotas over time. `enforce_quota` counts only active records;
  expiry/invalidation retain both maps. Repeated mint → expire → mint grows the store indefinitely and
  makes each quota check scan its entire history. The surviving temporary owner must delete expired
  items and release their resources, not accumulate terminal records.
- `HandlePolicy::max_expansion_bytes` is validated but never read by `revalidate` or `expand`.
  A target and requested read larger than that limit can pass their current size checks. Put the
  effective byte ceiling and request deadline on the actual retained read; a setting alone is no limit.
- The expansion code compares returned digest fields. Reuse the
  [single actual-byte verifier](../../../docs/implementation/README.md#turn-a-point-into-evidence);
  do not add another validator or accept expected digests beside arbitrary bytes.
- Replace the shared store and daemon `ResultHandleCatalog` with source references or optional item
  locators in the [one window owner](../search-continuation/README.md#one-temporary-owner). Update real
  mint/expand and native invalidation consumers together. Trusted-local construction needs no
  synthetic grant; selected restricted clients still require real scope/revocation/disclosure checks.
