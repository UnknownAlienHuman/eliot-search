# Lexical search: native ranking without losing exact-search semantics

The selected numerical encoder/index is Qdrant `qdrant/bm25`, server/client 1.19.0. Eliot may retain a small
code-term preparation helper. This is a proposed consumer cutover, not evidence that ranking quality or
large-corpus latency is acceptable.

## Existing source and actual callers

At public baseline `c8e38b6`, this package has about 205.6 KiB of source across the general analyzer,
frozen profiles, scoring-corpus builder, sparse encoders and code-identifier/qualification code. The legacy
daemon adds about 34.5 KiB of in-memory postings/BM25 code. Those are prior gross measurements, not a new
recount or a promised deletion size.

The previously traced production imports are daemon `lexical/{build,search,mod}.rs`, belonging to the old
snapshot application: it collects retained documents, builds local postings, scores locally and rebuilds
on refresh. The alternative frozen and code-identifier sparse APIs do not have connected production Qdrant
consumers. Do not preserve three numerical encoders, a second index or their profile-compatibility machinery.

Useful Unicode identifier splitting is independent of those engines. Keep it only with actual preparation
and query callers. Source coordinates stay with prepared/source evidence, not token-position receipts.

## One explicit native profile

```text
bounded retained unit + bounded selected context/aliases
-> Document(text, "qdrant/bm25", document options)
-> published named sparse vector

bounded request query + the same selected lexical preparation rules
-> Document(text, "qdrant/bm25", query options)
-> query under selected base population and pinned validity
```

Document and query roles are not interchangeable. Publish the chosen vector name and all behavior-affecting
options with the retrieval profile. Never inherit language defaults accidentally. For the initial word-based,
language-neutral code/text profile, make these choices explicit:

```json
{
  "tokenizer": "word",
  "lowercase": true,
  "ascii_folding": false,
  "stemmer": {"type": "none"},
  "stopwords": {},
  "k": 1.2,
  "b": 0.75,
  "avg_len": 256.0
}
```

The numerical values are a starting configuration from the donor, not tuned relevance evidence. Record any
selected token-length limits explicitly. `avg_len` is a supplied constant, not an automatically refreshed
corpus average. This tokenizer is not a universal segmentation solution for every language. A measured
multilingual or other profile is an explicit choice, never an automatic fallback. A semantic profile change
uses the publication generation boundary; do not relabel old vectors.

Qdrant's [actual 1.19 preprocessing](https://github.com/qdrant/qdrant/blob/v1.19.0/src/common/inference/bm25_inference.rs)
defaults to English when omitted. `language: "none"` is a deprecated side effect, not the chosen opt-out.
Native-vector readback follows the [bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md): the
server computes coefficients, and Eliot does not generate a second BM25 vector merely to compare them.

## Identifier expansion is recall, not identity

The [word tokenizer](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/field_index/full_text_index/tokenizers/mod.rs)
splits on non-alphanumeric characters. Thus these spellings have the same word tokens:

```text
parse_manifest     parse-manifest     parse::manifest
C++                C#
```

Keeping a whole spelling in the input string does not preserve a whole-token exact key. This distinction
was required by old architecture S12.3 and must survive removal of the old lexical engines.

The small optional helper may preserve original text and append finite unique snake/camel/acronym parts,
using existing XID and token-local NFC support. It must not claim collision-free name matching, source
identity or semantic equivalence. Aliases affect term frequency and length: appending both `parse_manifest`
and `parse manifest` can repeat the same native terms. Bound the expansion and assess its relevance effect;
do not create a second token/IDF engine to simulate the donor.

Exact spelling/literal requests use retained-source exact matching independently of BM25 top-k. Exact symbol
resolution additionally needs actual definitions/occurrences; a text occurrence may be a comment or test.
When a real syntax/SCIP consumer supplies name fields, its keyword lookup may nominate exact names, followed
by source validation and explicit ambiguity. Do not add such payload fields or a resolver service without
that producer/consumer. Never silently substitute lexical similarity for an unavailable exact operation.

## Scoped IDF has a measurable cost

The base population includes selected sources and pinned validity; query-only narrowing is separate.
Required source isolation must not silently degrade to global statistics. Known empty population returns
empty without RPC; unknown IDs and unavailable inventory remain errors/gaps.

In pinned Qdrant 1.19, scoped IDF collects matching point IDs or a segment-sized bit mask and examines queried
postings before ranking. `top_k`, output size and payload indexes do not remove that population work. See
[donor source analysis](../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#filtered-idf-performs-population-work).
The absence of an Eliot-side corpus scan is not proof of a corpus-independent query cost.

Use the selected native path first with finite query/concurrency limits and actual service timeout handling.
Before a scale claim, compare cold/warm requests over small and broad scopes, with staged/retired points
retained. Measure end-to-end latency, backend work/memory and cancellation, not only returned hits. No new
statistics cache, local denominator database, per-request collection or automatic sharding scheme is selected.

## Real consumer cutover

Preparation emits actual bounded retrieval text/options. Projection retains those inputs in its existing
operation pages. The bridge sends native documents and queries; publication verifies known bindings and
required vector structure; query resolves nominations through retained evidence. A correct excerpt proves
its source, not the relevance or completeness of the ranked result.

After those callers move, remove the general analyzer/receipt, frozen numerical profiles, ScoringCorpusBuilder,
local sparse hashing/weighting/qualification, legacy daemon postings/BM25, unused dependency edges and their
exclusive fixtures. Do not restore old vector byte goldens or another encoder registry. Keep causal Unicode
splitting cases. Inline the remaining helper if a package no longer earns its boundary.

## Behavior to assess after implementation

Use concrete cases instead of a universal profile verdict: exact `parse_manifest` among punctuation variants;
camel/snake/acronym recall; `not` and other English stopwords retained; explicit code versus non-space-language
limits; correct document/query roles; alias repetition; scope/validity isolation; and selected-population cost.
A wrong or empty lexical result cannot prove absence. Run minimal changed-owner compiler/Clippy after code;
then exercise the real Qdrant path. No live relevance or performance result is claimed here.
