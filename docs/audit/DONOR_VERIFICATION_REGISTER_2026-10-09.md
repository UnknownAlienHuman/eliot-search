# Donor and reuse index

**Refreshed:** 2026-10-18 against source `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
This index distinguishes existing mechanisms, actual replacement opportunities and unselected candidates.
It is static source research, not Rust/native/live-Qdrant qualification or a dependency upgrade proposal.
Use [root rules](../../AGENTS.md) and the [execution protocol](../product/EXECUTION_PROTOCOL.md).

Use donor code to remove a mechanism, not to justify preserving every old Eliot abstraction. The
[current architecture](../architecture/README.md) retires baseline obligations that have no local product
need. Reuse existing owners before adding wrappers; keep exact source truth and actual I/O boundaries.

## 1. Existing mechanisms and actual reductions

| Mechanism / inspected source | Current use and remaining work |
|---|---|
| [BLAKE3 1.8.2](https://github.com/BLAKE3-team/BLAKE3/blob/df610ddc3b93841ffc59a87e3da659a15910eb46/src/lib.rs) | Already used by [shared digest owner](../../crates/search-contracts/src/digest.rs). Replace fake semantic producers there through bounded owner APIs; #316 adds the required exact-stream consumer path. Preserve preimage/profile and limits; do not buffer whole large objects or build another hashing framework. |
| [RustCrypto sha2 0.10.9](https://github.com/RustCrypto/hashes/blob/82c36a428f8d6f05f3bfccdedb243e9d1f85359d/sha2/src/lib.rs) | Already accepted by the same owner. Three copied `sha256.rs` files total 685 physical lines. #315 and separate document-worker/supervisor slices can replace algorithm execution, preserving SHA-256 RAW versus PARTS_V1 framing. The 685 lines are gross source inventory, not achieved net deletion. |
| [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/Cargo.toml) | Existing vendor client, not a new donor adoption. #262 removes copied public PointId/payload/schema/eligibility definitions in the bridge in favor of shared Eliot contracts; private vendor conversion and strict validation remain. #263 supplies actual effects. `qdrant/qdrant-client` is the Python repository; Rust source is `qdrant/rust-client`. |
| `unicode-ident =1.0.26`, `unicode-normalization =0.1.25` | Already used by [search-lexical](../../crates/search-lexical/Cargo.toml). XID18 and NFC17 profiles/range maps remain explicit. Do not plan deletion of a nonexistent custom XID/NFC engine. Normalization is not full case folding. |
| `pulldown-cmark =0.13.1`, `quick-xml =0.42.0` | Already used by [Markdown](../../crates/search-prep/search-materializer/src/markdown/parse.rs) and [JATS](../../crates/search-prep/search-materializer/src/jats/parse.rs). They supply parsing events; local raw-span, bounds, cancellation and DTD/PI rejection remain product duties. No duplicate CommonMark/XML engine was established. |
| `redb =2.6.3` | Already used by [search-control-redb](../../crates/search-control-redb/Cargo.toml). Use its transactions for local control state. Retire the staged legacy migration platform and old daemon `control_store` with their actual consumers. A general migration/rollback state machine is not a dependency of redb. |
| Reusable provider client and native Windows owners | [Typed client](../../crates/search-provider-client/), [Windows connection](../../crates/search-runtime/search-provider-transport-windows/) and [purpose-bound credential adapter](../../bins/eliot-search/src/native_credentials.rs) exist. #332 connects actual commands over owned child stdio or an OS-owned pipe. The unconnected trusted-registration/listener path belongs to the optional restricted-client profile. [Current linkage/deletion map](../../bins/eliot-search/FUNCTIONS.md); reuse the engine and the smallest concrete transport that serves the local result. |
| S11 / indexed owner output | Existing planner/publication use S11. #262 consumes the planner's shared payload CBOR and separate vector evidence; old bridge revision/ordinal/digest fields are not rename-compatible S9.5 inputs. [Exact bridge disposition](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#shared-payload-and-identity-cutover). Preserve private vendor conversion. The [indexed donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md) prevents reimporting already delivered stacks. |

Exact workspace pins/features live in [Cargo.toml](../../Cargo.toml) and checksums in
[Cargo.lock](../../Cargo.lock). Existing crypto acceptance/provenance is retained in
[CRYPTO_DONOR_ACCEPTANCE](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md). The accepted BLAKE3 profile remains
`default-features=false`, `std,pure,zeroize`; upstream places `pure` among unstable/testing features, so
keep the exact pin. `sha2` remains default-features off; no second SHA implementation or compression API.
Do not add mmap/rayon/runtime file helpers to the shared authority package.

## 2. Architecture donors read in source

| Primary source inspected | Mechanism to use / Eliot consequence |
|---|---|
| [ripgrep 14.1.1 main](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | `main` passes one parsed `HiArgs` to concrete mode dispatch and search workers. Capture Eliot argv/config once and pass the resulting invocation; remove chains of recapturing `maybe_run` handlers. Its walker/searcher division is useful; its non-indexed app is not the replacement product. |
| [Tantivy 0.22.0 basic search](https://github.com/quickwit-oss/tantivy/blob/17d5869ad61ea9f1072ef40546ddcada3a14b067/examples/basic_search.rs) | One writer/commit and a cheap per-query immutable searcher snapshot, followed by TopDocs and stored-document read. Transfer the concrete snapshot/readback shape; keep Qdrant as Eliot's backend. A tutorial does not solve Eliot's publication/recovery itself. |
| [Qdrant Rust client 1.19.0 example](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/README.md#making-requests) | Concrete client → collection → bounded upsert → filtered query. The pinned client already exists. Eliot's [blocking facade](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs) already owns a reusable Tokio runtime; wire it instead of creating another runtime/registry. Do not copy the tutorial's destructive collection reset or treat its payload as source evidence. |
| [Serde 1.0.229 example](https://github.com/serde-rs/serde/blob/v1.0.229/README.md#serde-in-action) | Derive typed serialization for ordinary JSON records instead of repeated manual escaping/field tables. Serde/serde_json are already locked dependencies. Bound incoming frames and semantic sizes; keep identity-bearing canonical byte profiles distinct. |
| [Tokio 1.53.1 timeout source](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | `timeout_at` consumes an absolute instant. Convert one deadline at admission and propagate it across query/readback/pages. Current bridge `OpContext` stores a duration and restarts a timeout per RPC; more per-ABI counters do not fix that. |

These are source-backed design comparisons, not benchmark claims or new dependency approvals. Do not
copy a donor's entire framework. Existing `globset`, matching and parser libraries already cover many
primitive operations. The largest reduction here is removing unused requirements/owners, not another
package-name substitution.

## 3. Git: replace a small parser; complete the missing backend

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

## 4. Canonical bytes and Unicode decisions

The existing [closed canonical codec](../../crates/search-contracts/src/canonical.rs) owns bounded
JSON/CBOR and strict parse/re-encode equality. Its CBOR ordering is RFC 8949 **§4.2.3 length-first**, not
§4.2.1 core deterministic ordering. Existing identity-bearing bytes must not be silently reinterpreted.
However, preserving every pre-release internal encoding is no longer a requirement: use a new format/
generation and rebuild derived state when simpler library serialization replaces it. Keep a specific old
reader only for real retained data/consumers. Do not add another local canonical-codec package.

Ciborium 0.2.2 is not accepted as a production drop-in. Its prior source/API assessment remains in the
[frozen verification record](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).
It may be a differential oracle. The prior rejection only establishes that it is not a byte-compatible
drop-in for the old profile; it does not justify keeping handwritten codecs for every new internal
record. A new-format adoption can be evaluated on the simpler requirements. No Ciborium production
adoption, new dependency pin or wholesale codec replacement is claimed here.

[#253 / PR #322](UNICODE_CASEFOLD_DECISION_2026-10-09.md) already selected exact Unicode 18 C+F full
folding and goldens. Default folding excludes T; Turkic behavior is a distinct profile. No implicit extra
normalization; expanded scalars retain original source ranges. #254 remains the production consumer.
Do not reopen the completed decision, use floating UCD data, or substitute simple regex folding.

## 5. Further donor records: read the relevant section only

| Role | Existing owner / record |
|---|---|
| TOML / globs / matching | #238 typed TOML; #241 explicit globset settings; #246/#247 memchr/aho-corasick; #248 bounded regex-automata. [Detailed pinned source/profile record](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md). Recheck actual source before claiming an engine remains to be adopted. |
| Cargo / Rust syntax | Reuse actual Cargo metadata. #214 retires obsolete map/status policy and the custom partial Rust source analyser; keep private adapter visibility and review the real API. Do not build a pseudo-compiler or controller. |
| Code facts / documents / external corpora | [Completion map](../product/PROJECT_COMPLETION.md#4-preserve-optional-scope-explicitly) and existing #223–#236/#216 issues retain selected producer/profile boundaries. No speculative universal parser, connector framework or second index. |
| Historical indexed code | [Donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md) preserves exact #207/#209/#210/#200 heads, delivered portions and rejected ownership. |

Old Wave-1/Wave-2 “next”, `DECISION_REQUIRED` and coding-base statements are dated evidence, not current
adoption status or a mandatory queue. The existing [source audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md)
is the measured reduction map; avoid creating another registry to track this index.

## 6. Adoption without another process catalogue

Use the existing issue/PR to name the selected version/API/features, what code it replaces and any real
format or runtime limitation. Review new dependency/license/target changes when they occur; reuse the
existing accepted dependency evidence otherwise. Do not require another ecosystem audit, donor gate or
transitive-symbol dossier merely to use a library already present. The result is a connected caller and
a removed old mechanism, verified for its actual behavior.
