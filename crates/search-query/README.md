# Query and results

The [implementation guide](../../docs/implementation/README.md) connects retained sources, a published view,
real retrieval and evidence. That connection is necessary, but returning a verified excerpt is not by itself
useful code search. Old architecture S12.3, S16, S21-S25 contained user behavior that must survive deletion of
its recipes, grants, fingerprints and proof packages.

## Target flow

```text
explicit operation + selected sources + limits
-> pin published view and resolve non-widening scope
-> reserve finite candidate/output capacity and any session-window allowance
-> run the concrete handler for that operation
-> establish final candidate order and select a page
-> resolve exact evidence metadata and verify retained data
-> source/revision/location/excerpt, match basis and truthful gaps
-> optional window on the same live session
```

The operation determines what the result means. BM25 is one ranked nomination mechanism, not the
implementation of every request. A valid citation proves where the excerpt came from, not that it answers
the question, resolves a symbol, exhausts a corpus or represents independent implementations.

Text locations identify their retained representation and a half-open byte range `[start, end)`. Display
lines are 1-based; text columns use UTF-8 byte offsets unless another encoding is explicitly named. An adapter
using UTF-16 must label that encoding. Decoded/extracted text carries its source mapping and loss/gaps;
prepared-text offsets and page coordinates must never be presented as raw-source byte offsets. The actual
reader validates the stated range against that retained representation before emission.

## Product behaviors that must not disappear

| User task | Observable result | Smallest connected mechanism / limit |
|---|---|---|
| Find a literal or exact spelling | Actual matching ranges under declared byte/text semantics | [Exact reader/matcher](search-exact/FUNCTIONS.md), not filtering a truncated BM25 result |
| Locate a selected symbol | Exact source location, or bounded materially different definitions with reasons | Explicit location or actual syntax/semantic name observations; no same-name identity guess |
| Explore an unfamiliar repository | Useful entry points, headings/definitions and relevant next source reads | Existing prepared facts and source metadata; no architecture verdict generated from filenames |
| Search a question | Ranked relevant source excerpts, with the basis and missing evidence visible | [Native lexical query](../search-lexical/FUNCTIONS.md); optional stronger retrieval only when connected and assessed |
| Compare selected implementations | Side-by-side evidence for requested axes and explicit unknowns | Selected source ranges and real parser/semantic observations; no arbitrary behavioral equivalence |
| Check absence | No-match only for the stated predicate/domain and completely scanned inventory | Exact scan; a missing symbol provider or empty top-k is not absence |
| Inspect changing work | Which retained revision/view was used and what is known about pending observation/indexing | Current state/status already owned by source/publication; no fresh-workspace claim from a pinned old view |

These are product requirements, not orders for seven new services. The first lexical milestone can omit
unimplemented modes, but must not advertise them under a broad `Search` name or silently reinterpret them.
Deletion of a package does not close these behaviors as delivered.

## Scope comes from the published view

Resolve configured or explicit sources from one pinned view, optionally narrowed by a real restricted
session. Unknown IDs fail. Explicit empty and known empty inventory return an empty result without wildcard
RPC; unavailable/incomplete inventory is different. Compatible sources share the selected retrieval/IDF
population. [Lexical](../search-lexical/FUNCTIONS.md) owns the exact IDF meaning and donor cost caveat.

Publication establishes the immutable view's invariants. Ordinary deltas validate changed source/evidence rows
against the retained state; a full generation validates its captured population as it is built. Do not replace
per-query scans with a new unconditional whole-population scan on every small publication. Ordinary work turns
check selected scope and actual evidence against the pinned view, not the whole access/membership bundle.
Per-object integrity and genuine restricted-session revocation still apply. Hold a shared current-view lock
only while acquiring/installing the reference, never through backend work, source reads or transport output.
Do not import the old live-authority mutex/updater into this path.

Published is not necessarily current working tree. New source heads and unresolved watcher/indexing gaps
remain distinguishable from the revisions served. A client asking for current-workspace semantics must get
that actually established scope or a stale/incomplete/unsupported result, not a renamed published snapshot.
Do not reread the whole corpus on ordinary Search, infer freshness from watcher silence, or prove whole-scope
freshness from a few returned hits. Explicit synchronization and historical search are distinct operations.

Already admitted local requests may finish on their old immutable view. A selected restricted-session
revocation cancels that actual session/windows; no local grant or phase-checkpoint platform is reinstated.

## Direct operations, not a recipe operating system

Implement direct handlers: `Search`, `Exact`, `Continue`, `Expand`. Add `Orient`/`Compare` only with their
actual source-backed producers. Availability comes from construction, not capability descriptors.

Exact input does not become a lexical query on failure. An explicit location that no longer resolves does
not silently select a different symbol with a similar name. For an unqualified symbol name, keep distinct
definitions ambiguous unless real context disambiguates them. A raw text occurrence is not necessarily a
definition; syntax is not compiler-resolved call/reference evidence. The useful small selection logic in
[subject resolver](search-subject-resolver/FUNCTIONS.md) may be moved into its real consumer; no public
resolution ladder, receipt or service registry is needed.

For orientation use prepared paths/headings/definitions and independently observed imports/tests/docs where
available. For comparison choose actual source-backed subjects first, then show available evidence for each
requested axis. Absence of a fact is unknown unless the relevant exact operation established it. Equal names
or lexical scores do not establish equal behavior. Known copies may share a presentation group, but repeated
forks/copies must not be reported as independent confirming evidence. Do not invent repository lineage when
no producer supplies it.

Within one lexical population preserve server order. If results need source diversity, use the selected
native grouping or a finite, explicit presentation policy. Deduplication/reranking cannot recover candidates
that never entered the bounded candidate set. Do not promise enough distinct sources merely because a
larger raw hit limit was requested. Across incompatible scoring populations do not sort raw scores together.

## Cost is not the response length

Selecting a page before evidence reads avoids reading unselected candidates. It does not make the backend
or integrity verification O(page size). Qdrant 1.19 scoped IDF enumerates a population; a whole-object hash
or AEAD reader can require more bytes than the excerpt. Charge actual source/integrity work and expose a
limit gap rather than forging verification from a digest field. Group selected reads by object where useful.
No speculative IDF cache, range-authentication tree, secondary index or global scheduler is selected here.

Candidate/window admission precedes retrieval allocation, not just evidence reading. Follow the
[window owner's reservation rule](search-continuation/FUNCTIONS.md#admission-before-expensive-work);
no separate quota service is required. This bounds retained/transient request data, not all internal backend work.

Measure realistic scopes and source sizes through the connected application. A small successful example
establishes integration, not the large-corpus requirement. Remove scope-wide scans performed only for old
catalogue fences, but do not claim zero corpus-dependent work merely because it moved into a donor.

## One query window

Use the [window recipe](search-continuation/FUNCTIONS.md), not another handle/continuation store. It owns the
same pinned view, final references, handled-prefix cursor, fixed expiry and finite retained bytes. One-shot
CLI output cannot expose a window destroyed on child exit. Each Continue/Expand has a new request deadline
bounded by window expiry. Complete local output commits the handled prefix; it does not prove peer receipt.
Expansion is bounded context around an emitted item, not arbitrary revision offsets.

## Current owners and cutover

At inspected source `5af8cde5`, these are integration seams, not delivered operations:

| Behavior | Existing producer/helper | Receiving consumer and limit |
|---|---|---|
| Exact | [literal matcher](search-exact/src/literal.rs) over already supplied UTF-8 chunks | Target: read the pinned inventory through the retained reader; reuse the matcher as described in its [recipe](search-exact/FUNCTIONS.md). No BM25 shortlist or old proof/spine prerequisite. |
| Orientation / symbols | Source metadata and prepared text; [code-enricher](../search-prep/search-code-enricher/FUNCTIONS.md) has no connected outline consumer | Query/CLI can show observed paths/headings and gaps. Definitions, callers and relations need an actual producer; missing facts stay unsupported/unknown. |
| Comparison | [comparator](search-comparator/FUNCTIONS.md) assembles supplied observations; no connected production producer/readback caller | Select subjects and retained evidence in query first, then present supported axes. No claim of compiler-resolved behavior from this helper. |
| Documents | Markdown/XML parsers in preparation; [document worker](../../bins/eliot-search-doc-worker/FUNCTIONS.md) remains a refusal stub | Use supported prepared text/coordinates in the same evidence path. PDF extraction needs a selected real extractor; the worker name is not support. |
| Models / MCP | [model worker](../../bins/eliot-search-model-worker/FUNCTIONS.md) remains a refusal stub; no selected MCP runtime | Optional adapters consume the same connected operations. No empty worker, raw-Qdrant tool or new protocol catalogue is required for baseline delivery. |

Concrete scope, backend call, retained reader and result formatter replace the disconnected
recipe/capability/plan/grant/executor/proof stack. Existing packages are donor locations, not a required
package topology. Current contracts, daemon/client handlers and result consumers change together. Remove
obsolete forwarders and fixtures; no V3 compatibility family.

#279 owns connected Search, #249 Exact, #281 source-backed navigation/comparison, and #275 the window.
Those assignments should link here rather than reproduce another behavioral specification.

## Verification

After implementation and minimal changed-owner Clippy, use a small set of actual source tasks with known
locations: punctuation-colliding names; two same-named definitions; definition versus mention; implementation
plus relevant test/docs; known copies versus independent alternatives; raw versus decoded no-match; and a
changed source whose published revision is still old. Add real scoped-IDF and evidence-read costs before a
scale claim. Ordinary source examples and recorded outcomes suffice; no new evaluation/governance platform
or automatic all-feature gate is needed. No runtime, relevance or exhaustive-audit result is claimed here.
