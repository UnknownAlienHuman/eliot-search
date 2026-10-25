# Test and harness cutover

Tests preserve selected product behavior, not pre-release topology, receipts, waves or disconnected applications.

**Historical source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Current implementation-readiness checkpoint:** R047.

## Disposition rule

For each legacy test:

1. **Delete** when it freezes source layout, package topology, receipts, obsolete features or a removed state
   machine.
2. **Move the causal case** when it exposes behavior still owned by the current design.
3. **Retain unchanged** only when it already executes selected behavior through a real consumer.

Do not preserve an obsolete API to keep its fixture. Exact current caller inspection belongs to the code cut,
not another repository-wide audit.

## Evidence order

### During each production cut

Run only:

- the smallest locked Rust 1.98 `check` closure for changed production targets;
- minimal Clippy for those targets and immediate consumers;
- one cheap focused causal case when the implementation now exists.

### Foundation smoke

```text
help/version performs no owner effects
→ bounded bootstrap settings reach the actual root owner
→ inspect absent/current/unsupported without writes
→ initialize/open one plain current root
→ second owner is refused
→ close stores before dropping DataRootGuard
```

### First connected product smoke

```text
initialize/open plain current root
→ explicitly select one supported file
→ retain and reopen ObjectRef
→ prepare one result
→ build one fresh Qdrant generation
→ one CLI Search
→ pinned evidence lookup
→ exact retained excerpt
```

This smoke precedes broad reconciliation, encrypted storage, live sessions and cleanup.

### Convergence cases

1. restart/reopen exact root, object, source/prepared/view references;
2. stale source/preparation completion and A→B→A/remove→add;
3. lost wake-up reconciliation;
4. explicit `insert_only` identical retry after lost response;
5. foreign/conflicting point refusal;
6. fixed non-creating closure retry and readback;
7. old view remains exact while the new sequence closes points;
8. rebuild/compaction to a never-reused generation;
9. retired generation is not targeted by new publication;
10. collection deletion waits for all guards, uses terminal delete, verifies exact logical absence, never reuses
    the name and keeps cleanup/backpressure on uncertainty;
11. logical collection absence is not reported as physical disk erasure.

The pinned Qdrant 1.19 server/client path must execute these cases before release claims. Documentation and
unit-level request construction are not evidence of backend race behavior.

### Later cases

- Exact complete/incomplete semantics;
- live Continue/Expand and partial-output fail-stop;
- stopped-root local-object collection;
- archive/import isolation;
- selected optional formats/adapters;
- relevance and scale on the connected product.

## Current removal set

Remove with last callers:

- wave1–wave6 acceptance/features and source-shape gates;
- config fingerprints/activation receipts;
- root leases/owner slots;
- duplicate object writers;
- old planner/executor/handle/continuation stores;
- publication proof/compensation/per-point reclaimer stacks;
- pairing/loopback/legacy CLI sessions;
- unavailable optional workers with no selected consumer.

Useful causal requirements move to their receiving owner: no-clobber object publication, conservative incomplete
inventory, ABA-safe current rows, source-backed evidence, atomic view install, bounded framing/cancellation and
owned-child cleanup.

R031 check/Clippy covered materializer + unitizer only. R032–R047 documentation changes add no Rust/runtime
evidence.
