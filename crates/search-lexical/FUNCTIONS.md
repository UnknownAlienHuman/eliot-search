# Function contract — `search-lexical`

**Status:** W3/P06 logical contract; no encoder implementation or accepted profile exists yet.

The crate owns deterministic local sparse-vector encoding only. It stores no corpus and implements no
inverted index. One collection generation selects one accepted lexical provider path and profile.

## Profile operations

### `describe_profile() -> LexicalProfileDescriptor`

Returns the complete immutable behavior identity: provider artifact/version/digest, tokenizer,
Unicode normalization, identifier expansion, term-index mapping, collision strategy, weighting/BM25
parameters, Qdrant sparse modifier/schema and compatibility-fixture digest.

### `validate_profile(profile, qualification) -> Result<AcceptedLexicalProfile, LexicalError>`

Requires an accepted P06 qualification receipt and golden document/query fixtures. `latest`, implicit
defaults and partially specified profiles are rejected.

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
Failures include profile mismatch/unqualified provider, unsupported input, collision threshold failure,
non-finite weight and budget exhaustion.

## Required fixtures

Code/neutral golden tokens and vectors, Unicode/identifier/path cases, no implicit stopword/stemmer,
document/query compatibility, collision corpus, deterministic ordering, no double IDF, and
provider-path change requiring a new generation.
