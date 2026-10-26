# Test and evidence cutover

Tests preserve connected product behavior, not package layout, waves, receipts or disconnected applications. Run
the smallest locked Rust 1.98 compiler/Clippy closure after each production cut; add runtime evidence only for
behavior which exists.

**Current documentation checkpoint:** R052 over inspected head `87c84c24`.  
**Current compiler evidence:** R031 materializer + unitizer only. R032–R052 are documentation-only.

## Stage 1 — Foundation

After code exists:

- check/Clippy actual CLI/daemon/root/config/store consumers;
- help/version with zero root effects;
- inspect absent/current/incomplete/foreign/busy;
- manifest-last initialize and exact reopen;
- strict incomplete discard without foreign/user-data deletion;
- second-owner refusal and shutdown order;
- Windows handle-bound no-clobber canary: destination exists, path/parent replacement, crash debris, final readback;
- Unix no-replace/fallback and truthful durability.

Path-based `tempfile::persist_noclobber` documentation is not Windows handle-safety evidence.

## Stage 2 — one-file walking skeleton

Upgrade actual Rust client/server/schema to one exact Qdrant 1.20 patch, then execute:

```text
stable file read
→ ObjectRef/current SourceRecord
→ one prepared artifact + retrieval_text units
→ fresh vectorless one-shard collection
→ exact physical Text index V1
→ InsertOnly points + exact readback
→ deterministic TextQuery readiness canary
→ evidence/current commit + PublishedView
→ TextQuery with QueryScoringProfileV1
→ verified retained excerpt in default CLI
```

### Exact physical schema canary

Read back and compare every field:

```text
retrieval_text
TokenizerType::Code
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords=None
min_token_len=None
max_token_len=None
nested=None
scoring=BM25
enable_hnsw=false
memory=Pinned
vectors_config=None
one ordinary shard
```

Point/query response checks:

- explicit InsertOnly + wait;
- no vectors in points or response;
- response payload contains only source/evidence routing keys, not retrieval text/excerpts;
- exact point/payload/count readback;
- readiness TextQuery returns expected fixed order.

### Token/query semantics canary

- Code tokenizer splits snake/camel/Pascal/acronym transitions and preserves documented letter+digit behavior;
- ranking query receives subwords plus joined camel/snake identifiers;
- no duplicate local aliases in retrieval text;
- free TextQuery matches any query token;
- typed full-text filter matches all filter-query subwords in any order, not joined exact identifier/phrase;
- punctuation-sensitive Exact stays independent;
- one-file whole collection omits filter/IDF.

### Query-scoring canary

Baseline fixes `k=1.2`, `b=0.75`. Verify client rejects NaN/infinite/out-of-range values before dispatch and accepts
finite `k>=0`, `0<=b<=1`. Changing k/b changes request scoring without schema rebuild. No baseline score threshold.

Baseline tests must not require sparse BM25 inference, local avg_len/postings, alias expansion, occurrence matching,
stable cross-generation IDs, sequence, aliases, custom shards, watcher, encryption or another backend.

## Stage 3A — correctness/reference convergence

Add restart/reopen, complete desired-input capture, authoritative inventory, dropped-wakeup repair, edit/removal,
complete next generation, old pinned view, commit/install recovery and whole-collection cleanup.

Add only real filter indexes. Read back exact keyword/text params, `enable_hnsw=false` and explicit
`Pinned|Cached|Cold` memory. Prefix/tenant optimization appears only with a measured query shape.

Scope cases:

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

Verify base clauses in candidates and IDF, request-only clauses absent from IDF, explicit empty without RPC,
unknown/incomplete no widening and filter fields indexed.

Record predeclared corpus-class measurements:

- full build and edit→view p50/p95;
- points/payload bytes/backend writes;
- peak CPU/RAM/handles;
- text/filter index residency/cache behavior;
- current + building + reader-retained collection disk;
- redb retained-page growth;
- update arrival versus rebuild throughput;
- cleanup backlog and max reader/window lifetime.

Stage 3A ships only where every correctness and numeric budget passes.

## Stage 3B — only where Stage 3A budgets fail

Then test source-local merge, unambiguous occurrence reuse, operation-fixed births, InsertOnly additions, fixed
non-creating closures, exact sequence if retained, pending recovery, old-view evidence, text-statistics/optimizer
convergence after point updates and measured compaction/write reduction.

If all target classes pass Stage 3A, close Stage 3B issues `not_planned` with measurements.

## Later stages

- literal Exact and one serviced framed QueryWindow session;
- stopped-root collection/archive/import;
- source-backed orientation/comparison/Git/documents;
- optional shipped profiles only: encryption, regex, PDF/OCR, MCP, models, overlay, managed Qdrant or isolated
  backend replacement.

## Legacy-test disposition

1. Delete tests which only freeze source layout, packages, waves, receipts or removed state machines.
2. Move causal behavior to the current owner.
3. Retain unchanged only when a test already executes selected current behavior through a real consumer.

The two mixed migration suites already have complete move/delete dispositions. Do not re-audit them.

Delete rather than regenerate:

- sparse BM25 document/query vector goldens and local avg_len/postings equivalence;
- fixture-backed analyzer qualification/receipts;
- mandatory alias expansion/profile hashes;
- hidden server-default schema assumptions;
- stable occurrence identity/sequence in Stage 2/3A;
- exact count/completeness for ordinary top-k;
- public scheduler/ticket/leg/fusion/terminal maps.

## Result discipline

Every result names exact SHA, platform, toolchain, Qdrant client/server/profile, command and outcome. Use
`PASS | FAIL | PARTIAL | NOT_RUN`; missing evidence is not PASS. Compilation, source presence, schema/point readback
or issue closure alone does not prove scorer readiness, resource bounds, crash safety, relevance or end-to-end
behavior. R052 changes docs only.
