# search-qdrant-bridge

**C15 — sole Qdrant vendor/data-plane adapter.**

**Status:** substantial vendor-neutral oracle, live qualification harness and real `qdrant-client 1.19.0`
transport exist. The active `main` payload/filter surface still reflects the legacy membership/digest
contract and is not the canonical S9.5/S10.3 product generation. No live indexed capability is qualified
or enabled.

Draft PR #200 contains the aligned typed S9.5 payload, exact 19-index schema and single S10.3 retrieval /
filtered-IDF/count/scroll eligibility contract. It must be integrated with #207, #209, #210 and daemon
composition rather than partially copied into the legacy generation.

## Owns

- the only `qdrant-client` dependency and every vendor type translation;
- exact server/client/build qualification identity;
- authenticated loopback endpoint admission after the secret contract is resolved;
- collection creation, schema/index verification and strict-mode checks;
- bounded point mutation, readback, count, scroll and sparse query transport;
- wait/readback/strong-order semantics;
- exact response validation and stable Eliot-owned errors;
- possible-external-write `OUTCOME_UNKNOWN` classification;
- disposable live capability probes;
- in-memory behavioral oracle for tests only.

## Must not own

- executable/process/ACL/Job Object lifecycle;
- secret storage or plaintext credential persistence;
- recipe, access, publication, fusion or result semantics;
- source truth or evidence interpretation;
- vendor types in public ports;
- automatic download, upgrade, provider switching or oracle fallback.

## Product rules

- Qdrant is the only indexed/search database.
- Retrieval and `idf.corpus` use one canonical eligibility contract.
- Qdrant payload is metadata, never source evidence.
- Every emitted candidate is resolved through exact typed point readback and then authoritative source
  validation.
- A missing/mismatched collection schema, profile, route or qualification gate fails closed.
- Mutation timeout/cancellation after possible dispatch remains `QDRANT_MUTATION_OUTCOME_UNKNOWN` until
  exact reconciliation.

## Current blockers

- integrate the aligned S9.5/S10.3/S11 stack;
- resolve exact epoch range transport in #205;
- supply the Qdrant API key through the accepted redacted secret handoff (#201);
- wire the real bridge into daemon publication/query composition;
- execute real-server restart, noninterference, unknown-outcome and end-to-end qualification.

The in-memory bridge is never a production fallback, and a successful Qdrant CRUD smoke is not product
qualification.

The dependency and upgrade boundary is documented in
[`docs/runtime/QDRANT_ADAPTER_BOUNDARY.md`](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md).

- **Product area:** Architecture S9/S10
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Current status matrix:** [../../../docs/product/IMPLEMENTATION_STATUS.md](../../../docs/product/IMPLEMENTATION_STATUS.md)
