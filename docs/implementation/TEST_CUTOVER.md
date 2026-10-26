# Test and harness cutover

Tests preserve selected product behavior, not pre-release package layout, receipts, waves or disconnected
applications.

**Historical source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Current documentation checkpoint:** R048.

## Disposition rule

For each legacy test:

1. **Delete** when it freezes source layout, topology, receipts, obsolete features or a removed state machine.
2. **Move the causal case** when it exposes behavior still owned by the current design.
3. **Retain unchanged** only when it already executes selected behavior through a real consumer.

Do not retain an obsolete API for its fixture. Do not require broad suites before a connected path exists.

## Evidence order

### Each production cut

Run:

- smallest locked Rust 1.98 `check` closure for changed production targets;
- minimal Clippy for those targets and immediate consumers;
- one focused causal case only when the implementation exists and is cheap.

### Foundation

```text
help/version no effects
→ bounded root/plain settings
→ inspect absent/current/incomplete/foreign
→ manifest-last initialize
→ open one actual guard
→ factual status
```

Focused cases: zero-write inspect, final-manifest-last failure points, exact discard of recognized empty incomplete
layout, foreign/replacement refusal and second-owner exclusion.

### First connected smoke

```text
open plain root
→ select one supported file
→ retain/reopen ObjectRef
→ prepare once
→ build one fresh physical Qdrant collection with insert_only
→ commit evidence/current + install PublishedView
→ one CLI Search
→ exact retained excerpt
```

No restart, broad inventory, stable cross-generation point ID, fixed closure, watcher, session, cleanup or optional
format is required for this first result.

### Stage 3A — fresh-generation convergence

1. restart and exact reopen of root/object/source/prepared/current view;
2. startup/explicit/periodic inventory with incomplete-scan preservation;
3. source edit, removal and policy exclusion building a complete next generation;
4. dropped wake-up repaired from desired/published comparison;
5. old pinned view remains exact after new view installation;
6. crash between redb commit and view installation;
7. interrupted/foreign generation never becomes current;
8. whole-generation cleanup after last guard, name non-reuse and uncertain-delete retention.

### Stage 3B — only if measurement selects incrementality

1. unchanged/unique occurrence reuse and ambiguous duplicate reminting;
2. identical insert-only retry after lost response;
3. missing-point fixed-closure refusal;
4. closure while an older sequence view lives;
5. representative edits with measured point/write reduction versus 3A;
6. compaction threshold and fresh generation switch.

If 3B does not materially beat 3A on declared corpora, do not ship the incremental machinery merely because its
tests exist.

### Exact/session

Literal cross-block overlap, complete-negative only for fully scanned inventory, raw versus decoded representations,
partial frames, cancellation/EOF, output-before-cursor-commit and same-session Continue/Expand.

### Maintenance/depth

Stopped-root mark/sweep, archive/fresh import, then selected PDF/Git/orientation/overlay/model/MCP behavior only for
shipped profiles.

Relevance and scale follow the working path.

## Donor-derived canaries

Keep small causal tests for the failures seen in analogous systems:

- no manifest-first visibility of partially written artifacts (Veles counterexample);
- equal size+mtime does not prove unchanged bytes when a source is actually evaluated;
- walker/stat/ignore error cannot become complete inventory or deletion;
- Qdrant commit/current metadata and serving view installation are distinct (Tantivy reload lesson);
- no database/view/window mutex across embedding/backend/object/process I/O (qntx/qmd lock lesson);
- no Qdrant alias required for serving;
- existing content-addressed final is verified, never trusted from path alone (Git collision boundary).

These are owner tests, not a new analogue-conformance suite.

## Current removals

Remove with last callers:

- wave1–wave6 acceptance/features and source-shape gates;
- config fingerprints/activation receipts;
- root leases/owner slots/init journal/resume machine;
- duplicate object writers;
- old query planner/handle/continuation stores;
- publication proof/compensation/per-point-reclaimer stacks;
- alias-as-view logic;
- pairing/loopback/legacy CLI sessions;
- unavailable optional workers with no current consumer.

Useful causal requirements move to receiving owners: no-clobber object publication, conservative inventory,
ABA-safe rows, source-backed evidence, complete build then explicit view install, bounded framing/cancellation and
owned-child cleanup.

R031 check/Clippy covered materializer + unitizer only. R032–R048 documentation changes add no Rust/runtime
evidence.