# Test and harness cutover

Tests preserve selected product behavior, not pre-release package layout, receipts, waves or disconnected
applications.

**Historical source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Current implementation-readiness checkpoint:** R046.

## Disposition rule

For each legacy test:

1. **Delete** when it freezes source layout, package topology, receipts, obsolete features or a removed state
   machine.
2. **Move the causal case** when it exposes behavior still owned by the current design.
3. **Retain unchanged** only when it already executes the selected behavior through a real consumer.

Do not keep an obsolete API merely to preserve its fixture. Do not require broad suites before the connected
path exists.

## Evidence order

### During each code cut

Run only:

- the smallest locked Rust 1.98 `check` closure for changed production targets;
- minimal Clippy for those targets and immediate consumers;
- a focused causal case when the implementation now exists and is cheap to run.

### First connected smoke

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

This smoke precedes broad reconciliation, encrypted storage, live sessions, cleanup and optional formats.

### Next focused cases

1. restart and exact reopen;
2. stale source/preparation completion and A→B→A;
3. lost wake-up reconciliation;
4. identical insert-only retry and fixed closure retry;
5. old view during new publication;
6. rebuild/compaction and whole-generation cleanup;
7. Exact, Continue/Expand and cancellation;
8. stopped-root collection and archive/import.

Relevance and scale follow the working path.

## Current removals

Remove with last callers:

- wave1–wave6 acceptance/features and source-shape gates;
- config fingerprints/activation receipts;
- root leases/owner slots;
- duplicate object writers;
- old query planner/handle/continuation stores;
- publication proof/compensation/point-reclaimer stacks;
- pairing/loopback/legacy CLI sessions;
- unavailable optional workers with no consumer.

Useful causal requirements move to their receiving owner: no-clobber object publication, conservative incomplete
inventory, ABA-safe current rows, source-backed evidence, atomic view install, bounded framing/cancellation and
owned-child cleanup.

R031 check/Clippy covered materializer + unitizer only. R032–R046 documentation changes add no Rust/runtime
evidence.
