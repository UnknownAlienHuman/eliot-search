# Test and evidence cutover

Tests preserve connected product behavior, not pre-release package layout, receipts, waves or disconnected
applications. Run the smallest Rust 1.98 compiler/Clippy closure after each production cut; add focused runtime
evidence only for behavior which now exists.

**Current documentation checkpoint:** R049 over inspected head `fed10c1e`.
**Current compiler evidence:** R031 materializer + unitizer only. R032–R049 are documentation-only.

## Evidence order

### Stage 1 — Foundation

After root/config code exists:

- locked check and Clippy for actual CLI/daemon/root/config/store consumers;
- help/version with zero root effects;
- inspect absent/current/incomplete/foreign/busy;
- final-manifest-last initialize and exact reopen;
- strict `discard-incomplete` without foreign/user-data deletion;
- second-owner refusal and shutdown ordering;
- Windows handle-based no-clobber finalization canary, including destination-exists and path/parent replacement;
- truthful durability reporting.

Do not accept `tempfile::persist_noclobber` documentation or a path-only rename as Windows evidence.

### Stage 2 — one-file walking skeleton

Upgrade the actual bridge/client/schema to Qdrant 1.20.x in the same code cut. Then run:

```text
one stable file read
→ plain ObjectRef
→ current SourceRecord
→ PreparedRef
→ fresh physical collection
→ scoring full-text index with explicit code/text processing
→ insert_only point build and exact readback
→ evidence/current commit and PublishedView install
→ TextQuery
→ retained excerpt in default CLI
```

Verify exact source/revision/location/excerpt, missing/corrupt evidence refusal, Qdrant order preservation, known
empty scope, bounded output, cancellation and help/status capability truth. Baseline tests must not require sparse
BM25 inference, fixed `avg_len`, aliases, custom shards, watcher, stable cross-generation point identity, encrypted
storage or a second backend.

### Stage 3A — correctness/reference convergence

Add restart/reopen, authoritative startup/explicit/periodic inventory, dropped-wakeup repair, source edit/removal,
complete next generation, old pinned view, commit/install crash recovery and whole-generation cleanup.

Record a measurement matrix for declared corpus classes:

- full build p50/p95;
- edit-observed → view-installed latency;
- points/payload bytes and backend writes;
- peak memory/CPU/handles;
- current + building + reader-retained generation disk;
- update arrival rate versus rebuild throughput;
- cleanup backlog and longest supported reader/window.

Stage 3A is a release mode only where #215's numeric budgets pass. These numbers are product/release inputs, not
values invented by a unit test.

### Stage 3B — only where Stage 3A misses budgets

Then test stable occurrence reuse, explicit insert-only additions, fixed non-creating closures, sequence-range
exactness if retained, pending-delta recovery, old-view evidence and measured compaction. If every target corpus
class passes Stage 3A, close Stage 3B issues `not_planned` with the measurements.

### Later stages

- literal Exact and one serviced framed live session/QueryWindow;
- stopped-root object collection, archive/import and selected product leaves;
- optional profiles only when shipped: encryption, PDF, Git, MCP, models, overlay, managed Qdrant or an isolated
  Qdrant Edge replacement spike.

Qdrant Edge is not a baseline test matrix while its selected released line remains beta/unqualified for ELIOT.
Custom sharding is not a baseline optimization because BM25 statistics are shard-local.

## Legacy-test disposition

1. **Delete** tests which only freeze source layout, package topology, waves, receipts, obsolete features or a
   removed state machine.
2. **Move causal behavior** to the receiving owner when the failure remains relevant.
3. **Retain unchanged** only when the test already executes selected current behavior through a real consumer.

The two mixed migration suites already have complete move/delete dispositions. Do not re-audit them. Remove
remaining wave1–wave6, config activation, legacy query/handle/proof and CLI/provider fixtures as their real caller
moves. A subprocess, native call or large fixture does not make an obsolete architecture current.

## Donor canaries live with owners

- Windows handle finalization canary belongs to root/object owners.
- Qdrant 1.20 scoring full-text/code-tokenizer/scope/cleanup cases belong to bridge/publication/query.
- `ignore` traversal/error cases belong to reconciliation.
- Edge or backend replacement experiments remain isolated and cannot become an all-target gate.

Do not create a repository-wide donor-conformance scanner, receipt service or qualification authority.

## Completion discipline

Every reported result names exact SHA, platform, toolchain, Qdrant server/client/profile, command and outcome.
Use `PASS | FAIL | PARTIAL | NOT_RUN`; missing evidence is not PASS. Compilation, source presence, readback or a
closed issue alone does not establish crash safety, relevance, bounded work or end-to-end behavior.
