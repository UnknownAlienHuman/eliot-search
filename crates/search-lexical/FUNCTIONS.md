# search-lexical — source/caller audit and reduction map

The current package contains three lexical designs while the selected product should use Qdrant's native
BM25 plus a small bounded code-term preparation helper.

## Measured source

At public main `c8e38b6`, `src/` is **205,607 bytes (~200.8 KiB)**:

| Family | Bytes | Actual role |
|---|---:|---|
| generic analyzer (`lib.rs`) | 30,559 | builds tokens, term statistics, original offsets and an analysis receipt |
| frozen profiles | 31,261 | pins old analyzer/hash/TF/IDF/collision parameters and canonical profile digests |
| scoring corpus | 21,444 | stores up to one million logical documents/frequency entries to derive another denominator digest |
| sparse encoder subtree | 44,129 | hash mapping, collision accounting, numerical weighting, vectors, profile/readback fingerprints |
| code identifiers core | 29,317 | XID scanning, NFC, snake/camel/acronym components plus profile/fixture identities and receipts |
| code identifier encoding/fixtures | 48,530 | second sparse numerical route, qualification and compatibility/collision corpora |
| façade | 367 | reexports every family |

The daemon also contains a separate **34,476-byte** in-memory lexical implementation under
`bins/eliot-searchd/src/lexical/{build,manifest,mod,search}.rs`.

## Exact caller reality

Repository source search finds imports of `search_lexical` in only three production Rust files:

```text
bins/eliot-searchd/src/lexical/build.rs
bins/eliot-searchd/src/lexical/search.rs
bins/eliot-searchd/src/lexical/mod.rs
```

Those files belong to the legacy snapshot/TCP-token application in `bins/eliot-searchd/src/main.rs`. They:

- capture/read complete retained documents into a private snapshot manifest;
- build a `BTreeMap<String, Vec<Posting>>` in memory;
- use the generic analyzer for both documents and queries;
- compute BM25 locally with fixed `k1=1.2`, `b=0.75`;
- reopen whole retained files to construct excerpts;
- expose separate `search:` and `lexical:` protocol commands;
- rebuild the complete snapshot/index on `refresh`.

This is a second search backend and a second application, not the Qdrant product path.

Other exact symbol searches show:

- `ScoringCorpusBuilder` appears only in its defining file;
- `code_v1_sparse_profile` appears only in `frozen.rs` and `scoring.rs`;
- `CodeIdentifiersProfile` and `encode_code_document/query` appear only in this package's source,
  documentation and fixtures;
- Cargo manifests for overlay/executor declare `search-lexical`, but source imports were not found.

Thus the general/frozen/sparse/code-encoding APIs have no external production consumer. Do not preserve them
as future contracts.

## Three incompatible lexical semantics

Public main already contains three different systems:

1. legacy daemon analyzer + local BM25 (`k1=1.2`, `b=0.75`);
2. frozen `code_v1`/`text_neutral_v1` hash-index profiles with logarithmic TF and delegated Qdrant IDF;
3. `code_identifiers@1` with Raw document TF, Binary query TF, seeded one-million-index mapping,
   qualification and collision fixtures.

They do not form one compatible profile. Keeping all of them would force profile migration and verification
for behavior the selected application never needs.

## Selected lexical path

Use the existing pinned Qdrant client/server local `qdrant/bm25` implementation:

```text
document unit
→ bounded retrieval text
→ Document(text, model="qdrant/bm25", document options)
→ selected named vector

query text
→ bounded query preparation
→ Document(text, model="qdrant/bm25", query options)
→ scoped native query with explicit idf.corpus
```

The published retrieval profile binds document/query roles, vector name, `k`, `b`, `avg_len`, tokenizer,
case, stemmer, stopwords and folding. A semantic change builds a new physical generation and switches the
published view after complete readback. Do not run a second local encoder to predict token IDs/vector bytes or
a vector digest.

Qdrant owns token hashing, TF/length weighting, native sparse storage and IDF over the selected base
population. Ordinary ranked top-k remains non-exhaustive. Exact/literal absence belongs to the retained-source
exact scanner.

## Small code-term preparation which may survive

Qdrant's word tokenizer does not implement Eliot's desired identifier decomposition. Preserve only a pure
bounded helper, conceptually:

```text
prepare_code_search_text(text, role, limits, deadline/cancellation)
    -> PreparedSearchText
```

Useful behavior:

- preserve the whole identifier;
- add unique bounded snake_case, camelCase/PascalCase and acronym components;
- use `unicode-ident` for XID boundaries;
- apply token-local NFC only to emitted search terms;
- keep deterministic ordering and a finite expansion ratio;
- use the same selected rules for document and query roles;
- return ordinary prepared text/terms and actual counts, not qualification receipts.

Source coordinates remain with the original unit/`ViewManifestEntry`; Qdrant evidence resolves the unit/range,
not token offsets. Exact identifier matching reads original source bytes. Therefore the helper does not need
the current source-revision/ordinal/absolute-range input, token-position arrays, term-statistics maps or
`LexicalReceipt`.

The current hard ceilings—8 MiB input, two million tokens/expansions and one million unique terms per call—are
not useful unit limits. Bound the already finite structural unit and output expansion before allocation.

Keep `search-lexical` only if this shared Unicode algorithm remains substantial and is used by both
preparation and query. Then its dependencies should be essentially the two Unicode crates plus small current
DTOs. Otherwise move the helper beside retrieval-input preparation and delete the package.

## Delete with caller cutover

- generic analyzer/token/term-statistics/receipt API;
- `frozen.rs` profiles, canonical profile preimages and pinned fingerprints;
- `ScoringCorpusBuilder`, `FrozenCorpusStatistics` and denominator receipts;
- complete `sparse/**` hash mapping, collision, weighting, vector and qualification machinery;
- code identifier sparse encoding, acceptance/qualification and fixture-identity machinery;
- 33.8 KiB compatibility/collision fixture implementation after useful term-splitting cases move;
- legacy daemon in-memory postings/BM25/snapshot lexical modules and their TCP/token commands;
- unused overlay/executor Cargo dependency edges;
- `search-domain`, `search-ports` and `search-config` dependencies from this package;
- old sparse bytes, profile/golden/collision/receipt tests and generated maps.

Gross direct removal candidate before replacing the useful helper is about **175.9 KiB in the crate** plus
**34.5 KiB in the daemon lexical module**. This is inventory, not promised net deletion.

## Caller cutover

1. Preparation produces one bounded retrieval text per stable unit, optionally using the small code helper.
2. Projection pages retain actual text/options rather than precomputed local sparse vectors/digests.
3. The Qdrant bridge accepts native `Document` input for point upsert and query.
4. Publication reads back actual selected vector presence/state and point identity.
5. Query uses the same published role/options and explicit base `idf.corpus`.
6. Remove the legacy snapshot lexical application and every old local sparse consumer.
7. Shrink or delete `search-lexical` after the helper has real direct consumers.

Do not add a lexical-provider trait, encoder registry, compatibility adapter or profile qualification service.

## Tests

Move only causal term-preparation cases:

- Unicode XID boundaries and token-local NFC;
- whole identifier plus bounded snake/camel/acronym components;
- duplicate expansion suppression and deterministic order;
- finite expansion/output limits and cancellation;
- matching document/query rules for the selected role profile.

Connected behavior tests must publish real prepared text through Qdrant and retrieve it under the selected
scope/IDF population. Delete local vector golden bytes, hash-collision qualification, denominator digests,
manual BM25 ordering and legacy snapshot/TCP fixtures with their implementations.
