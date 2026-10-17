# Donor and reuse index

**Refreshed:** 2026-10-17 against source `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
This index distinguishes existing mechanisms, actual replacement opportunities and unselected candidates.
It is static source research, not Rust/native/live-Qdrant qualification or a dependency upgrade proposal.
Use [root rules](../../AGENTS.md) and the [execution protocol](../product/EXECUTION_PROTOCOL.md).

A donor supplies a mechanism. ELIOT retains source/root admission, exact identity/profile, access,
currentness, limits, coordinates, publication, lifecycle and evidence authority. Use an existing owner
before inventing a wrapper or importing a framework. A lockfile entry alone does not qualify a profile.

## 1. Existing mechanisms and actual reductions

| Mechanism / inspected source | Current use and remaining work |
|---|---|
| [BLAKE3 1.8.2](https://github.com/BLAKE3-team/BLAKE3/blob/df610ddc3b93841ffc59a87e3da659a15910eb46/src/lib.rs) | Already used by [shared digest owner](../../crates/search-contracts/src/digest.rs). Replace fake semantic producers there through bounded owner APIs; #316 adds the required exact-stream consumer path. Preserve preimage/profile and limits; do not buffer whole large objects or build another hashing framework. |
| [RustCrypto sha2 0.10.9](https://github.com/RustCrypto/hashes/blob/82c36a428f8d6f05f3bfccdedb243e9d1f85359d/sha2/src/lib.rs) | Already accepted by the same owner. Three copied `sha256.rs` files total 685 physical lines. #315 and separate document-worker/supervisor slices can replace algorithm execution, preserving SHA-256 RAW versus PARTS_V1 framing. The 685 lines are gross source inventory, not achieved net deletion. |
| [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/Cargo.toml) | Existing vendor client, not a new donor adoption. #262 removes copied public PointId/payload/schema/eligibility definitions in the bridge in favor of shared Eliot contracts; private vendor conversion and strict validation remain. #263 supplies actual effects. `qdrant/qdrant-client` is the Python repository; Rust source is `qdrant/rust-client`. |
| `unicode-ident =1.0.26`, `unicode-normalization =0.1.25` | Already used by [search-lexical](../../crates/search-lexical/Cargo.toml). XID18 and NFC17 profiles/range maps remain explicit. Do not plan deletion of a nonexistent custom XID/NFC engine. Normalization is not full case folding. |
| `pulldown-cmark =0.13.1`, `quick-xml =0.42.0` | Already used by [Markdown](../../crates/search-prep/search-materializer/src/markdown/parse.rs) and [JATS](../../crates/search-prep/search-materializer/src/jats/parse.rs). They supply parsing events; local raw-span, bounds, cancellation and DTD/PI rejection remain product duties. No duplicate CommonMark/XML engine was established. |
| `redb =2.6.3` | Already used by [search-control-redb](../../crates/search-control-redb/Cargo.toml). Tables, journal conditions and migrations are application responsibilities. Remove the obsolete daemon `control_store` with its harness; do not merge unrelated encrypted-object and control-journal transaction protocols merely because both have receipts. |
| Reusable provider client and indexed owners | Existing Eliot source is the first donor: [client](../../crates/search-provider-client/), S11, UnitSet, planner and publication admission. Use the [indexed donor disposition](INDEXED_SPINE_PORT_MAP_2026-10-09.md); do not reimport closed stacks or recreate delivered crates. |

Exact workspace pins/features live in [Cargo.toml](../../Cargo.toml) and checksums in
[Cargo.lock](../../Cargo.lock). Existing crypto acceptance/provenance is retained in
[CRYPTO_DONOR_ACCEPTANCE](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md). The accepted BLAKE3 profile remains
`default-features=false`, `std,pure,zeroize`; upstream places `pure` among unstable/testing features, so
keep the exact pin. `sha2` remains default-features off; no second SHA implementation or compression API.
Do not add mmap/rayon/runtime file helpers to the shared authority package.

## 2. Git: replace a small parser; complete the missing backend

[#335](https://github.com/UnknownAlienHuman/eliot-search/issues/335) is a candidate adoption, not an
accepted version or a measured large deletion. Current [safe-reader Git code](../../crates/search-source/search-safe-reader/src/git.rs)
validates already-decompressed loose-object bytes from an abstract backend. It does not implement a
filesystem object store, pack/delta/ref reader or SHA-1 recomputation; packed and remote-required cases
remain explicit unsupported results.

Inspect narrow `gix-object` for object/header parsing and `gix-odb`/`gix-pack` only for required local
backend behavior. Upstream inspection is pinned to
[gitoxide ad0202e3](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-odb/src/lib.rs).
That API supports writes and follows alternate object directories; a crate name does not make it
read-only or confined. Admission must constrain every object/alternate path, replacement mapping,
format and read, with total compressed/expanded/object-count/depth/deadline/allocation bounds.

Select and verify a minimal exact release/feature closure at adoption. Exclude network/fetch/checkout,
hooks/filters/credentials, source writes, external `git` execution and all-in-one Git runtime defaults.
Object hash verification and immutable retained-source binding remain required. Added backend
functionality is implementation work, not credit for deleting code that never existed.

## 3. Canonical bytes and Unicode decisions

The existing [closed canonical codec](../../crates/search-contracts/src/canonical.rs) owns bounded
JSON/CBOR and strict parse/re-encode equality. Its CBOR ordering is RFC 8949 **§4.2.3 length-first**, not
§4.2.1 core deterministic ordering. Preserve exact accepted bytes; a profile change requires explicit
migration/rebuild. Do not create `search-canonical`, leak arbitrary donor values or substitute a generic
Serde codec without exact closed-profile proof.

Ciborium 0.2.2 is not accepted as a production drop-in. Its prior source/API assessment remains in the
[frozen verification record](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).
It may be a pinned differential oracle; no equivalently bounded byte-compatible replacement was
established by this pass. This is a compatibility limit, not permission for another local codec owner.

[#253 / PR #322](UNICODE_CASEFOLD_DECISION_2026-10-09.md) already selected exact Unicode 18 C+F full
folding and goldens. Default folding excludes T; Turkic behavior is a distinct profile. No implicit extra
normalization; expanded scalars retain original source ranges. #254 remains the production consumer.
Do not reopen the completed decision, use floating UCD data, or substitute simple regex folding.

## 4. Further donor records: read the relevant section only

| Role | Existing owner / record |
|---|---|
| TOML / globs / matching | #238 typed TOML; #241 explicit globset settings; #246/#247 memchr/aho-corasick; #248 bounded regex-automata. [Detailed pinned source/profile record](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md). Recheck actual source before claiming an engine remains to be adopted. |
| Cargo / Rust syntax | Existing metadata/AST owners #250/#251; #214 removes obsolete policy/derived-output coupling while preserving actual product guards. Do not build a pseudo-compiler or controller. |
| Code facts / documents / external corpora | [Completion map](../product/PROJECT_COMPLETION.md#4-preserve-optional-scope-explicitly) and existing #223–#236/#216 issues retain selected producer/profile boundaries. No speculative universal parser, connector framework or second index. |
| Historical indexed code | [Donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md) preserves exact #207/#209/#210/#200 heads, delivered portions and rejected ownership. |

Old Wave-1/Wave-2 “next”, `DECISION_REQUIRED` and coding-base statements are dated evidence, not current
adoption status or a mandatory queue. The existing [source audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md)
is the measured reduction map; avoid creating another registry to track this index.

## 5. Evidence for a new adoption

Record once in the existing issue/PR: exact package/role/version/source/checksum; license and bundled-data
terms; Rust 1.98/target compatibility; normal/build/dev dependency and feature closure; advisories;
required API/limits/cancellation facts; public-type boundary; byte/profile disposition; replaced paths
and last callers; focused fixture/oracle and qualification owner. Reuse valid accepted evidence instead
of re-auditing the ecosystem. The manager verifies load-bearing primary source and the extraction diff.
If a donor cannot meet a required bound/profile, document that concrete failure before selecting the
named fallback. No unreviewed lockfile churn or second implementation as a temporary shortcut.
