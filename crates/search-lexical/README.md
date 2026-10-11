# search-lexical

**C11 — deterministic lexical analyzer and sparse encoder.**

**Status:** implemented pure lexical analysis plus qualified sparse document/query encoding. No index or corpus store is owned by this crate.

Implemented behavior:

- bounded Unicode alphanumeric tokenization with exact original byte offsets;
- explicit case, underscore, minimum-length, stop-word and position-gap policy;
- deterministic term statistics;
- stable seeded term-to-index mapping;
- reject-or-measure collision policy with a finite rate threshold;
- raw, logarithmic and BM25 local TF weighting;
- binary, raw and logarithmic query TF weighting;
- explicit `None`, Qdrant-delegated, or frozen-local IDF mode;
- structural prevention of double IDF;
- sorted unique finite sparse vectors;
- exact profile/qualification/statistics/fingerprint receipts;
- no I/O, index mutation, provider selection, or hidden fallback.

A profile is unusable until `validate_sparse_profile` receives a matching accepted qualification receipt. Any tokenizer, mapping, weighting, IDF, collision, artifact or fixture change requires a different profile fingerprint and collection generation.

## Additive code identifiers profile

`CodeIdentifiersProfile::new()` owns immutable `code_identifiers@1`: XID Unicode18 from
unicode-ident1.0.26, token-local NFC Unicode17 from unicode-normalization0.1.25, and preserved case.
Its internally computed shared CBOR/BLAKE3 identity binds both published artifacts/data editions,
tinyvec1.13.3, complete grammar/limits, mapping/weighting and authored fixture definitions. It does
not advertise full Unicode18 normalization. Definition identities do not certify executed fixtures.

Maximal continuation runs require an XID-or-underscore start; invalid starts are skipped whole.
Whole identifiers precede proper underscore/ASCII camel/acronym components. Invalid-start components
are filtered after raw work charging; punctuation delimits runs without joined compounds. Siblings
share one parent position and NFC duplicates retain the first original span. Separate parents retain
occurrences. Original and normalized scalar/byte ceilings are checked independently, with all raw
attempts charged before normalization, filtering and deduplication. Lower operation limits cannot
exceed the profile's hard ceilings. Refusal returns no successful partial output.

`accept_code_identifiers_profile` checks the existing qualification record against the internally
computed profile, artifact-set descriptor and fixture-definition identities. Its accepted wrapper
feeds both `encode_code_document` and `encode_code_query` through one shared route, reusing the
legacy numerical bodies through a private parameter view. The fixed baseline uses Raw document TF,
Binary query TF, a seeded1048576-index mapping, measured merging with a10000ppm ceiling, and Qdrant
IDF exactly once. New-mode local statistics/BM25 are not offered by this precise profile. Qualification
must independently measure collisions and the document/query fixtures before product adoption.

Existing `AnalyzerConfig`, `analyze`, `SparseProfile` fields, sparse encoders and frozen code_v1/
text_neutral_v1 identities remain explicitly legacy and byte-compatible for real consumers. The new
mode has distinct opaque types; there is no conversion through a dummy legacy analyzer or caller
fingerprint. Downstream adoption requires VERSIONED_REBUILD. Daemon/Qdrant cutover and installed
qualification remain separate owners. New fixtures are AUTHORED_NOT_RUN under #215.

- **Delivery wave:** W3 / P06
- **Agent instructions:** [AGENTS.md](AGENTS.md)
