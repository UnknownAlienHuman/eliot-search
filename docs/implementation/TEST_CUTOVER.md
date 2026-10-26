# Test and evidence cutover

Tests preserve connected product behavior, not package layout, waves, receipts or disconnected applications. Run
the smallest locked Rust 1.98 compiler/Clippy closure after each production cut; add focused runtime evidence only
for behavior which exists.

**Current documentation checkpoint:** R051 over inspected head `a4c23110`.  
**Current compiler evidence:** R031 materializer + unitizer only. R032–R051 are documentation-only.

## Stage 1 — Foundation

After code exists:

- check/Clippy actual CLI/daemon/root/config/store consumers;
- help/version with zero root effects;
- inspect absent/current/incomplete/foreign/busy;
- final-manifest-last initialize and exact reopen;
- strict incomplete discard without foreign/user-data deletion;
- second-owner refusal and shutdown order;
- Windows handle-bound no-clobber canary: destination exists, parent/path replacement, crash debris, final readback;
- Unix no-replace/fallback cases and truthful durability reporting.

Path-based `tempfile::persist_noclobber` documentation is not Windows handle-safety evidence.

## Stage 2 — one-file walking skeleton

Upgrade actual Rust client/server/schema to one exact Qdrant 1.20 patch, then execute:

```text
stable file read
→ plain ObjectRef/current SourceRecord
→ one prepared artifact + units/retrieval_text
→ fresh vectorless physical collection
→ scoring Text index (Code tokenizer, explicit processing)
→ source/access filter indexes
→ insert_only points + exact readback
→ deterministic TextQuery readiness canary
→ evidence/current commit + PublishedView install
→ TextQuery
→ verified retained excerpt in default CLI
```

Focused cases:

- exact source/revision/location/excerpt and missing/corrupt evidence refusal;
- vectorless collection and exact schema readback;
- code tokenizer camel/snake/acronym plus joined identifier without local alias duplication;
- TextQuery any-term behavior;
- typed required/excluded filter clauses;
- whole-collection scope omitting redundant filter/IDF;
- punctuation-sensitive Exact remains separate;
- Qdrant order preservation, bounded output and cancellation;
- help/status advertise only constructed handlers.

Baseline tests must not require sparse `qdrant/bm25` inference, local `avg_len`, alias expansion, old/new occurrence
matching, stable cross-generation IDs, sequence ranges, aliases, custom shards, watcher, encryption or another
backend.

## Stage 3A — correctness/reference convergence

Add restart/reopen, complete desired-input capture, authoritative inventory, dropped-wakeup repair, source edit/
removal, complete next generation, old pinned view, commit/install crash recovery and whole-collection cleanup.

Scope cases use:

```text
base population = selected sources ∩ real access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Verify base filter is present in both candidates and IDF, request-only clauses are absent from IDF, explicit empty
returns without RPC, unknown/incomplete never widens, and filter fields are indexed.

Record predeclared corpus-class measurements:

- full build p50/p95 and edit-observed → view-installed p50/p95;
- point/payload bytes and backend writes;
- peak memory/CPU/handles;
- current + building + reader-retained collection disk;
- redb retained-page growth;
- update arrival rate versus rebuild throughput;
- cleanup backlog and maximum reader/window lifetime.

Stage 3A may ship only where every correctness and numeric budget passes.

## Stage 3B — only where Stage 3A budgets fail

Then test source-local descriptor merge, unambiguous occurrence reuse, operation-fixed births, insert-only additions,
fixed non-creating closures, exact sequence boundaries if retained, pending-delta recovery, old-view evidence,
optimizer/statistics behavior after point updates and measured compaction/write reduction.

If every target class passes Stage 3A, close Stage 3B issues `not_planned` with measurements.

## Later stages

- literal Exact and one serviced framed live session/QueryWindow;
- stopped-root object collection and archive/import;
- source-backed orientation/comparison, Git and documents;
- optional profiles only when shipped: encryption, regex, PDF/OCR, MCP, models, overlay, managed Qdrant or one
  isolated backend replacement spike.

## Legacy-test disposition

1. Delete tests which only freeze source layout, packages, waves, receipts or removed state machines.
2. Move causal behavior to the receiving current owner.
3. Retain unchanged only when the test already executes selected current behavior through a real consumer.

The two mixed migration suites already have complete move/delete dispositions. Do not re-audit them. Remove
remaining wave/config/root/query/handle/proof/CLI-provider fixtures as their real caller moves.

Specific obsolete lexical/query cases to remove rather than regenerate:

- sparse BM25 document/query vector goldens and fixed `avg_len`;
- local postings/numerical encoder equivalence;
- fixture-backed analyzer acceptance/qualification;
- mandatory alias expansion/profile hashes;
- stable occurrence identity in Stage 2/3A;
- exact count/completeness for ordinary top-k;
- public scheduler/ticket/leg/fusion/terminal maps.

## Donor canaries live with owners

- filesystem finalization canaries belong to root/object owners;
- Qdrant 1.20 vectorless/text-index/TextQuery/IDF/cleanup cases belong to bridge/publication/query;
- ignore traversal/error cases belong to reconciliation;
- parser cases belong to materializer;
- backend replacement experiments stay isolated and cannot become an all-target gate.

No repository-wide donor-conformance scanner, receipt service or qualification authority.

## Result discipline

Every result names exact SHA, platform, toolchain, Qdrant server/client/profile, command and outcome. Use
`PASS | FAIL | PARTIAL | NOT_RUN`; missing evidence is not PASS. Compilation, source presence, point readback or a
closed issue alone does not establish scorer readiness, crash safety, relevance, bounded work or end-to-end
behavior.
