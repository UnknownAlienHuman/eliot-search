# Unicode full case-fold decision — 2026-10-09 (#253)

Publication base: `c098346e32322462872d4ec589c1121128689d9c` (merged #237).
Research began at `25616fa4d422815ce9402c4d703d4b63129fe9b3`. Research/data only; no
lexical Rust, dependency, tokenizer or product qualification is delivered here.
Primary data, archive checksums, fixture output and standards revisions were
independently verified against the exact retained sources listed below.

## Selected profile

Select a checked-in generated table, with no production case-fold crate.
`casefold_full_18.0.0_r1` uses Unicode 18.0.0 C+F mappings, excludes S and T,
and leaves unlisted scalars unchanged. A separate explicitly selected
`casefold_turkic_18.0.0_r1` replaces the mappings for U+0049 and U+0130 with T.
The default never consults language, OS locale or ambient state.

The normalization choice is **none before or after folding** for both profiles.
NFC and NFKC are separate possible future profiles and cannot be introduced under
these identifiers. This is an explicit profile choice: folding does not preserve
normalization. Composed and decomposed spellings can have different emitted bytes.
Neither this decision nor its fixtures claim canonical caseless equivalence.

For #254: apply the identity normalization stage, segment original UTF-8 using
the separately reviewed UAX #29 adapter, then fold each selected word's scalars.
Segmentation always precedes folding. No stemming, language detection, stop list
or extra minimum-length filtering is enabled by this fold decision. UAX #29
release/adapter acceptance remains an explicit #254 dependency review; the fold
algorithm, data, normalization and original-range policy require no further choice.

Every emitted term retains the complete original token byte range, including
all original bytes responsible for an expansion. Individual output scalars do
not acquire fabricated source offsets. Do not segment transformed output or
derive source ranges from its length. A range that cannot be represented exactly
is rejected. Document and query encoding select the identical profile.

## Primary evidence

Retrieved/reviewed on 2026-10-09:

- [CaseFolding 18.0.0](https://www.unicode.org/Public/18.0.0/ucd/CaseFolding.txt):
  88,776 bytes, header date 2026-02-03, SHA-256
  `a004797658a457bec4dc11683e39f69249ea3b595b752dbea6721c4c9f587b0d`.
- [UAX #44 revision 38](https://www.unicode.org/reports/tr44/tr44-38.html):
  Unicode 18.0.0, dated 2026-09-02.
- [UAX #15 revision 58](https://www.unicode.org/reports/tr15/tr15-58.html):
  Unicode 18.0.0, dated 2026-08-12.
- [UAX #29 revision 49](https://www.unicode.org/reports/tr29/tr29-49.html) and
  [UAX #31 revision 45](https://www.unicode.org/reports/tr31/tr31-45.html):
  Unicode 18.0.0, dated 2026-09-01. Identifier normalization is #252's separate owner.
- [Unicode License V3](https://www.unicode.org/license.txt): SHA-256
  `e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96`.

The exact primary data and complete license notice are retained under
`fixtures/unicode-casefold/`. Redistribution includes that notice; generated
tables must retain the Unicode attribution and data checksum. Standards HTML
is referenced, not redistributed. A future generator runs offline against the
retained input; neither builds nor product execution download Unicode data.

Independent counts: C=1501, F=105, S=32, T=2; the default table has 1606 entries.
Each F output expands to multiple scalars; the maximum output is three scalars.
Supplementary-plane mappings are included. C+S describes simple folding and is
insufficient for this profile.

## Exact generator plan and ceilings

#254 emits one sorted static array of records `(input: u32, output: [u32; 3],
length: u8)` for all 1606 C+F mappings, plus a separate two-record T array.
Unused array positions are zero padding, never emitted U+0000. Identity is the
fallback only when binary search finds no record. Scalar values, unique input
keys, lengths 1..=3 and ascending order are verified before generation.

The #254 implementation generator must parse the pinned primary file, ignore
comments, select C/F, reject duplicate selected inputs, emit deterministic Rust
literals/version/hash constants, and differentially replay every selected mapping. It also checks
that all absent entries remain identity and that T overrides exactly two inputs.
Do not import a comparison-only crate's private table or add compression before
an explicit representation review. There are at most 1608 records and at most
three emitted scalars per input scalar; a checked conservative byte ceiling is
12 output bytes per input scalar, followed by the stricter term limits.

Both source-token and emitted-term scalar ceilings are 512; emitted UTF-8 is at
most 1024 bytes, matching the existing baseline lexical limits. Check additions
and reject before output growth; never truncate. Existing input/token/unique-term
and position limits still apply. A scalar expansion is a fold result, not a
new independently positioned token. The uncompressed table's numeric fields
account for 27,336 bytes before target padding; actual source/binary cost and
Rust 1.98 conformance are measured by #254, not claimed by this decision.

Changes to data, policy, tables, ceilings or fixture identity require a new
profile revision and lexical/projection generation. Rebuild old vectors from
retained source; do not relabel legacy `UnicodeLowercase` or rewrite old identity.

## Candidate source review

The manager recomputed archive hashes and inspected exact release sources.
No candidate is a production dependency; no MSRV/build result is invented.

| Candidate | Exact archive SHA-256 | Proven source fact and disposition |
|---|---|---|
| focaccia 2.5.0 | `d0d565312085d6ae0832a9f9efd76adf0983b924128f6ebe752009f63e70b8ee` | Commit `1b08dcb17d9e4a5c0cf20645fa4bf86f1cfb6acc`, MIT AND Unicode-3.0, MSRV 1.85, no normal/build dependencies. Its Unicode 18 primary data matches our hash, but `folding::mapping` is private and public folding APIs compare rather than emit terms. Rejected production; possible future differential oracle. |
| unicode-casefold 0.2.0 | `b7f66b1c8f8caa2ab31dc6d3f35386f16efdab89668f93411e565ac368908e8f` | MIT OR Apache-2.0, no normal dependencies, no declared MSRV. Emits folds but table declares Unicode 9.0.0. Rejected stale data. |
| caseless 0.2.2 | `8b6fd507454086c8edfd769ca6ada439193cdb209c7681712ef6275cccbfe5d8` | Commit `f77d542c5439252f75d8da2f0cb4a13d3a13c5ea`, MIT, no declared MSRV, normal `unicode-normalization` dependency. It does emit default folds, but its table is Unicode 16.0.0. Rejected stale data; comparison helpers also contain additional normalization. |
| casefold 0.1.0 | `4382156a4e6d5f0c279184bb37207f1259a5061c23084d3aa93eedf2a68c2eed` | Commit `b8c6e27652f7f198a3a0f2183179948cd69be0c1`, MIT, build script and no normal/build dependencies. Source explicitly selects simple C+S and omits F/T. Rejected semantics. |

Reviewed registry release dates: focaccia 2026-10-03, unicode-casefold 2016-10-28,
caseless 2024-12-30, casefold 2026-07-09; these exact releases were not yanked.
The local RustSec snapshot `7eebec69c352c7191b1f13eb95dd510eeca5d1de`
has no package record matching those four names. This is a bounded snapshot
screen, not a workspace audit or proof of absence of vulnerabilities.
ICU4X is not needed because the primary-table option satisfies emitted-fold and
range requirements; its dependency stack is not accepted or qualified here.
Rust lowercase and simple/regex folding remain rejected substitutes for full F.

## Goldens and verification boundary

`fixtures/unicode-casefold/casefold_goldens.tsv` contains 39 deterministic
fold-level cases: ASCII, sharp S, all sigma/I variants, combining and
composed/decomposed cases, supplementary mappings, empty input and the exact
512-scalar/1024-byte boundaries plus overflow. Status is recorded per original
scalar; both default and Turkic scalar/UTF-8 outputs are present. Whole-input
ranges in these fold fixtures become token ranges after #254's segmentation.
`ACCEPT` means the fold candidate fits limits, not that punctuation or a lone
combining mark is an emitted UAX #29 word.

SHA-256 of the exact LF UTF-8 TSV:
`464f5185ff3852f7aa4c4a697d2a2eb8aed538a2f38dbbdf6fb94ce37a837af7`.
`generate-goldens.ps1` is an offline research-fixture generator; it is not a
runtime analyzer or build script. The data hash is checked before parsing.
The fixture directory's `.gitattributes` disables text conversion for the pinned
data, license and TSV. Their exact LF bytes and checksums survive Windows Git
checkouts; the generator itself uses an explicit LF checkout profile.
The fixture hash, exact data checksum, policies, limits and adapter release must
enter #254's internally computed profile identity through #237.

These data/generator fixtures were produced by the manager. SA-06 independently
re-derived all 39 rows, checked all 16 fields and whole-original byte ranges,
and regenerated a byte-identical TSV from the checksum-checked input. SA-01
independently confirmed the primary table counts and exact data hash. The PR
records their review disposition. No lexical implementation, crate
compilation, segmentation conformance, benchmark or installed product is claimed.
