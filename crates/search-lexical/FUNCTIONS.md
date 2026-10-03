# Function contract — `search-lexical`

**Status:** substantive deterministic analyzer and sparse document/query encoding source exists. No exact
lexical profile is yet accepted and installed in the supported live Qdrant product path. Source presence is
not P06/W3 qualification.

The crate owns pure local lexical preparation. It stores no corpus, implements no inverted index and
performs no Qdrant I/O. One collection generation selects one exact qualified lexical profile; runtime
fallback or implicit provider switching is forbidden.

## Implemented source surface

The active crate exports:

- bounded lexical `analyze` over exact UTF-8 units;
- exact original/source byte offsets and position gaps;
- deterministic term statistics;
- accepted sparse-profile validation through `validate_sparse_profile`;
- stable term mapping and collision measurement;
- document/query weighting;
- `encode_document` and `encode_query`;
- sorted unique finite `SparseVector` values;
- content-free fingerprints and receipts;
- explicit `None`, Qdrant-delegated or frozen-local IDF modes.

The implementation still requires one accepted profile, golden fixtures and real daemon/Qdrant
composition before lexical capability may be advertised.

## Profile contract

A `SparseProfile` and `SparseQualification` must bind every behavior-affecting field:

- provider/profile identity and revision;
- analyzer configuration and fingerprint;
- Unicode/case/token-character policy;
- identifier expansion and term mapping;
- collision policy and measured ceiling;
- document/query TF weighting;
- local versus Qdrant-delegated IDF;
- dimensions, finite limits and golden fixture identity.

`validate_sparse_profile` rejects a mismatched qualification. `latest`, implicit defaults, floating
artifacts and partially specified profiles are inadmissible. Any behavior change requires a new profile
fingerprint and normally a new collection generation.

## Encoding contract

```text
encode_document(input, accepted_profile, statistics, lexical_limits, sparse_limits, cancelled)
    -> Result<SparseEncoding, SparseError>

encode_query(input, accepted_profile, statistics, lexical_limits, sparse_limits, cancelled)
    -> Result<SparseEncoding, SparseError>
```

Successful output contains:

- deterministic lexical analysis and mapped features;
- sorted unique sparse indexes;
- finite values;
- profile/analyzer/input/feature/vector fingerprints;
- bounded token, feature and collision accounting;
- exact qualification receipt reference;
- statistics identity when the profile requires it.

Cancellation, malformed input, non-finite weights, collision-limit failure or budget exhaustion return no
partial vector represented as valid.

## IDF rule

The accepted profile declares which TF/length factors are local and whether corpus IDF is delegated to
Qdrant. Applying corpus IDF twice is forbidden. Product retrieval and `idf.corpus` must use the same
eligibility population.

## Agent-retrieval extension

The concrete code/text projection families, deterministic query expansion and fusion profile required for
ordinary agent questions are tracked by #221. The public free-text/orientation contract is tracked by
#213. Neither task may add another search database or accept raw Qdrant plans from clients.

## Required qualification

- code and neutral-text golden document/query tokens and vectors;
- Unicode, snake/camel/Pascal, qualified-name and path cases;
- no implicit stemming, stop-word removal or semantic synonym expansion;
- deterministic mapping/order/fingerprints;
- collision corpus and threshold verdict;
- no double IDF;
- package check and strict Clippy at the exact revision;
- real Qdrant filtered-IDF and end-to-end candidate/readback evidence;
- profile change forcing a new collection generation.

Until that evidence exists, the crate is `SOURCE`, not `QUALIFIED` or `ENABLED`.
