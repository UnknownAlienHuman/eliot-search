# search-qdrant-bridge

Qdrant collection, point and query operations behind private vendor translation.
Oracle, qualification harness and real transport source exist; the shared S9.5/S11 cutover remains
#262. Source presence and static artifact/configuration admission do not establish live qualification.
[FUNCTIONS.md](FUNCTIONS.md) records the exact remaining contract mismatch and required behavior.

The bridge owns schema/capability probes, strict indexed filters, exact mutation/readback and vendor
error mapping. Process/ACL/Job Object lifecycle belongs to the supervisor; secret storage, access,
publication and result authority remain in their owners. No automatic upgrade, download or fallback.

The [current architecture](../../../docs/architecture/README.md) retires mandatory test-report tokens
from connection and the exact count from ordinary top-k. Keep actual endpoint/auth/schema checks and
source validation. `BlockingRealQueryPlane` already has one reused Tokio runtime per instance.

## Implemented transport and fixture boundaries

- The T22 disposable-server harness owns only qualification process setup, exact artifact measurement,
  bounded diagnostics and cleanup. Product lifecycle remains in `search-qdrant-supervisor`.
- Startup diagnostics read at most the final 1 KiB of each captured log. Lossy UTF-8 is confined to
  failure evidence; the harness never loads an unbounded log or emits product authority.
- Live probes retain one mandatory order for exact identity, topology/index/schema, wait/strong CRUD,
  strict negative behavior, signed range, sparse modifier, independent IDF and missing upper bounds.
  IDF comparisons use exact ID/score populations without assuming tie ordering.
- Exact readback rejects duplicate IDs and returns points/missing/unexpected IDs in deterministic order.
  Filtered nominations preserve ranking order while rejecting duplicate IDs and non-finite scores.
- Mutation/schema errors distinguish definite pre-dispatch cancellation from possible durable effects.
  Transport loss after possible write yields `QDRANT_MUTATION_OUTCOME_UNKNOWN`; read transport loss
  remains `QDRANT_TRANSPORT_FAILED`. Collection existence without usable readback is not success.

The current private codec still writes legacy payload/vector-digest fields and accepts numeric/UUID
point responses; the current physical collection name is a truncated 28-byte derived locator.
These are existing implementation facts, not the accepted S9.5/S11 or durable route authority.
The [cutover contract](FUNCTIONS.md#shared-payload-and-identity-cutover) defines their replacement;
private Qdrant value conversion and exact readback remain necessary.

## Verification boundary

The [adapter boundary](../../../docs/runtime/QDRANT_ADAPTER_BOUNDARY.md) retains Cargo/API behavior
checks and retires the custom source analyser. The old diagnostic command is:

```text
cargo run --locked -p xtask -- validate qdrant-boundary --json
```

Source-shape/module-name assertions are retirement candidates; the disposable suite's actual behavior
cases remain useful evidence. The [execution protocol](../../../docs/product/EXECUTION_PROTOCOL.md) selects source gates
and defers test/live execution until the integrated candidate. [AGENTS.md](AGENTS.md) owns package
guidance; historical waves and line targets do not require another wrapper or file split.
