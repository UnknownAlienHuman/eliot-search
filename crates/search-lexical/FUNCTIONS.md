# Function contract — `search-lexical`

**Status:** legacy analyzer/sparse source and additive code_identifiers@1 source are present. The
code-identifiers owner executes its frozen runtime qualification fixtures before issuing an
`AcceptedCodeIdentifiersProfile`; source presence or identity matching alone is not qualification.
Runtime qualification is distinct from configured-profile state and `ReleaseQualified` evidence.

The crate owns deterministic local sparse-vector encoding only. It stores no corpus and implements no
inverted index. One collection generation selects one accepted lexical provider path and profile.

## code_identifiers@1 source operations

- `CodeIdentifiersProfile::new`: immutable internally computed complete shared CBOR/BLAKE3 identity;
  separately pinned XID18/NFC17 and the exact tinyvec closure. No caller fingerprint/donor override.
- `CodeIdentifierLimits::new`: positive lower operation ceilings within the identity-bound hard maxima.
- `CodeIdentifiersProfile::analyze`: bounded maximal XID continuation runs, valid underscore/XID starts,
  whole then proper snake/adjacent-ASCII camel/acronym spans, token-local NFC and exact original ranges.
  Invalid parent runs are discarded whole; invalid-start components are filtered after attempt charge.
  Parent positions/counts and raw expansion counts are separate; dedup is per parent only.
- `accept_code_identifiers_profile`: compare profile/artifact-set/fixture-definition identities and the
  existing qualification contract; matching alone is not execution or release evidence.
- `qualify_code_identifiers_profile`: run every frozen compatibility case, the exact document/query
  index and weighting fixture, and the complete collision corpus under one caller-owned absolute
  deadline/checkpoint. It compares token normalization/spans, sparse indices and values, feature
  mapping, and collision report against frozen owner outputs. It returns the opaque accepted profile
  only after all checks pass; it does not issue configured-profile or `ReleaseQualified` state.
- `encode_code_document` / `encode_code_query`: same borrowed input and immutable accepted profile,
  one private new route and shared private numerical bodies, Raw document/Binary query/Qdrant IDF.
  `encode_code_document_with_checkpoint` adds a caller-owned absolute deadline and live observer to
  that same kernel. No new local-statistics/BM25 mode. Old legacy numerical options and public APIs
  remain unchanged.
- `validate_code_identifiers_pair`: compare owner-produced new document/query kinds and profile identity.
  Collection-generation selection/authorization remains with the projection owner.

Artifact-set identity hashes an exact canonical descriptor of published SHA256 release/table facts;
it is not a relabelled BLAKE3 archive checksum. Compatibility/collision identities bind the frozen
definitions, while `qualify_code_identifiers_profile` separately executes and checks their owner
outputs. The operational bootstrap is not #215 product, scale, external acceptance or
`ReleaseQualified` evidence. New profile adoption requires VERSIONED_REBUILD; real legacy
daemon/frozen consumers remain separate.

## Profile operations

### `describe_profile() -> LexicalProfileDescriptor`

Returns the complete immutable behavior identity: provider artifact/version/digest, tokenizer,
Unicode normalization, identifier expansion, term-index mapping, collision strategy, weighting/BM25
parameters, Qdrant sparse modifier/schema and compatibility-fixture digest.

### `validate_profile(profile, qualification) -> Result<AcceptedLexicalProfile, LexicalError>`

Requires an accepted P06 qualification receipt and golden document/query fixtures. `latest`, implicit
defaults and partially specified profiles are rejected.

### `qualify_code_identifiers_profile(profile, lexical_limits, sparse_limits, deadline, checkpoint)`

Runs the code-identifiers owner's frozen runtime cases through the same analyzer, mapping, numerical
weighting and collision implementations used for ordinary document/query encoding. The expected
normalization, sparse index, document/query weight, and collision outputs are owner-frozen data; the
qualifier does not derive its expectations from the values it is checking. Cancellation, deadline,
limit failure or any expected-output mismatch returns no accepted profile. This is operational runtime
qualification only; configuration admission, `ReleaseQualified`, independent scale evidence and release
acceptance remain separate.

### `profile_digest(profile) -> LexicalProfileId`

Hashes canonical profile bytes under the lexical-profile domain. Any behavior change yields a different
profile ID and requires a new collection generation.

## Encoding operations

### `normalize_input(input, profile, budget) -> Result<NormalizedLexicalInput, LexicalError>`

Applies NFC and the profile's exact case rules. It never guesses language, applies ASCII folding,
stopwords or stemming unless the accepted profile explicitly says so.

### `expand_identifier(token, profile) -> BoundedTokenSet`

For the code profile emits the raw identifier plus configured snake, camel/Pascal, qualified-name and
path components. Expansion is deterministic, duplicate-free and bounded.

### `tokenize_document(input, profile, budget) -> Result<TokenSequence, LexicalError>`

### `tokenize_query(input, profile, budget) -> Result<TokenSequence, LexicalError>`

Document and query tokenization may differ only where the profile and compatibility fixture explicitly
define it. Unsupported modality/encoding returns a typed failure.

### `map_terms(tokens, profile) -> Result<SparseFeatureSet, LexicalError>`

Maps terms to stable sparse indexes using the accepted vocabulary/hash policy. Collisions follow the
declared strategy and are measurable; they never establish exact identity or absence.

### `weight_document(features, statistics, profile) -> Result<SparseVector, LexicalError>`

### `weight_query(features, profile) -> Result<SparseVector, LexicalError>`

The profile declares which TF/length factors are local and which IDF factor is delegated to Qdrant's
accepted sparse modifier. Applying corpus IDF twice is forbidden.

### `encode_document(input, profile, budget) -> Result<LexicalEncoding, LexicalError>`

### `encode_query(input, profile, budget) -> Result<LexicalEncoding, LexicalError>`

Return sorted, unique sparse indexes, finite numeric values, profile ID, input digest, token/feature
counts and a bounded non-content receipt. Outputs are deterministic and size-bounded.

### `measure_collision_corpus(corpus, profile, budget) -> Result<CollisionReport, LexicalError>`

Produces measured collision rates and threshold verdicts without exposing corpus content in ordinary
telemetry.

## Configuration operations

`section_descriptor`, `compiled_defaults`, `validate_section`, `section_digest` and
`plan_section_change` implement `config/sections/lexical.md`. No lexical profile change is live; it
requires an accepted profile plus new collection generation.

## Cancellation, retry and failure

Encoding is pure and retry-safe. Budget or cancellation returns no partial vector advertised as valid.
The live code-document entry point polls after at most 64 tracked analysis, mapping, weighting,
validation or fingerprint work units, and at aggregate allocation/result boundaries. Its absolute
`Instant` deadline is fixed for the call; deadline expiry returns `SPARSE_ENCODING_CANCELLED`, while
observer errors propagate unchanged.
Failures include profile mismatch/unqualified provider, unsupported input, collision threshold failure,
non-finite weight and budget exhaustion.

## Required fixtures

Code/neutral golden tokens and vectors, Unicode/identifier/path cases, no implicit stopword/stemmer,
document/query compatibility, collision corpus, deterministic ordering, no double IDF, and
provider-path change requiring a new generation.
