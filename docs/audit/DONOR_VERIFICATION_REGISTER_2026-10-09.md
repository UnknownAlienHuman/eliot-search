# Donor and reuse index

**Refreshed:** 2026-10-18 against source `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
This index distinguishes existing mechanisms, actual replacement opportunities and unselected candidates.
It is static source research, not Rust/native/live-Qdrant qualification or a dependency upgrade proposal.
Use [root rules](../../AGENTS.md) and the [implementation guide](../implementation/README.md).

Use donor code to remove a mechanism, not to justify preserving every old Eliot abstraction. The
[current architecture](../architecture/README.md) retires baseline obligations that have no local product
need. Reuse existing owners before adding wrappers; keep exact source truth and actual I/O boundaries.

## 1. Existing mechanisms and actual reductions

| Mechanism / inspected source | Current use and remaining work |
|---|---|
| [BLAKE3 1.8.2](https://github.com/BLAKE3-team/BLAKE3/blob/df610ddc3b93841ffc59a87e3da659a15910eb46/src/lib.rs) | Already used by [shared digest owner](../../crates/search-contracts/src/digest.rs). First remove unnecessary plan/receipt hashes. Retained identity/integrity uses the real bounded owner, including #316's exact-stream consumer work; do not buffer whole large objects or reinterpret an existing digest as a different algorithm/profile. |
| [RustCrypto sha2 0.10.9](https://github.com/RustCrypto/hashes/blob/82c36a428f8d6f05f3bfccdedb243e9d1f85359d/sha2/src/lib.rs) | Already accepted by the same owner. Three copied `sha256.rs` files total 685 physical lines. #315 and separate document-worker/supervisor slices can replace algorithm execution, preserving SHA-256 RAW versus PARTS_V1 framing. The 685 lines are gross source inventory, not achieved net deletion. |
| [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/Cargo.toml) | Existing vendor client, not a new donor adoption. #262 removes copied public PointId/payload/schema/eligibility definitions in the bridge in favor of shared Eliot contracts; private vendor conversion and strict validation remain. #263 supplies actual effects. `qdrant/qdrant-client` is the Python repository; Rust source is `qdrant/rust-client`. |
| `unicode-ident =1.0.26`, `unicode-normalization =0.1.25` | Already used by [search-lexical](../../crates/search-lexical/Cargo.toml). XID18 and NFC17 profiles/range maps remain explicit. Do not plan deletion of a nonexistent custom XID/NFC engine. Normalization is not full case folding. |
| `pulldown-cmark =0.13.1`, `quick-xml =0.42.0` | Already used by [Markdown](../../crates/search-prep/search-materializer/src/markdown/parse.rs) and [JATS](../../crates/search-prep/search-materializer/src/jats/parse.rs). They supply parsing events; local raw-span, bounds, cancellation and DTD/PI rejection remain product duties. No duplicate CommonMark/XML engine was established. |
| `redb =2.6.3` | Already used by [search-control-redb](../../crates/search-control-redb/Cargo.toml). [Database API](https://github.com/cberner/redb/blob/v2.6.3/src/db.rs) supplies a single writer and read snapshots; [transactions](https://github.com/cberner/redb/blob/v2.6.3/src/transactions.rs) default to Immediate durability. Use typed current-record methods, not a second general journal/receipt platform. redb does not make files and Qdrant one transaction; retain actual pending publication, not every completed update. |
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
| [ripgrep 14.1.1 main](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | `flags::parse` produces one `HiArgs`, then `run` selects a concrete mode. In Eliot's [entry](../../bins/eliot-searchd/src/entry.rs), replace recapturing `maybe_run` chains and repeated config/argv fingerprints with one parsed invocation. Preserve `OsString` paths. This organisation needs no new CLI framework or dependency. |
| [Tantivy 0.22.0 basic search](https://github.com/quickwit-oss/tantivy/blob/17d5869ad61ea9f1072ef40546ddcada3a14b067/examples/basic_search.rs) | One writer/commit and a cheap per-query immutable searcher snapshot, followed by TopDocs and stored-document read. Transfer the concrete snapshot/readback shape; keep Qdrant as Eliot's backend. A tutorial does not solve Eliot's publication/recovery itself. |
| [Qdrant Rust client 1.19.0 points](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs), [query](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/query.rs) | Existing `upsert_points`, `get_points` and `query` plus Eliot's [reused blocking runtime](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs). Produce bounded batches before calling the SDK: `upsert_points_chunked` receives the entire points vector and copies while splitting; it is not streaming ingestion. Remove duplicate bridge contracts/receipt ownership. The SDK does not replace source validation or publication; do not copy a tutorial's destructive collection reset. |
| [Serde 1.0.229](https://github.com/serde-rs/serde/blob/v1.0.229/README.md#serde-in-action), [serde_json 1.0.151 decoder](https://github.com/serde-rs/json/blob/v1.0.151/src/de.rs), [encoder](https://github.com/serde-rs/json/blob/v1.0.151/src/ser.rs) | Derives, `from_slice` and `to_writer` replace Eliot's [manual schema codec](../../crates/search-provider-protocol/src/frame/client/wire.rs), fixed-order cursors/escaping and [substring event parsing](../../bins/eliot-search/src/public_client.rs). Versions exist in Cargo.lock; these production codecs still need direct dependency integration. Bound the frame before parsing and bound output. `from_reader` waits for EOF, so it is unsuitable as-is on a persistent connection. No separate JSON grammar or frozen field order is required for ordinary records. |
| [Tokio 1.53.1 timeout source](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | `timeout_at` consumes an absolute instant. Convert one deadline at admission and propagate it across query/readback/pages. Current bridge `OpContext` stores a duration and restarts a timeout per RPC; more per-ABI counters do not fix that. |

These are inspected mechanisms, not benchmark or runtime claims. Direct Serde integration is proposed;
redb/Qdrant/Tokio already have production adapters. Existing [globset](../../crates/search-source/search-source-admission/src/policy_v2.rs)
and [aho-corasick](../../crates/search-query/search-exact/src/literal.rs) have real callers. Reuse them;
do not import donor applications, another backend or another layer around each API.

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
§4.2.1 core deterministic ordering. These are facts about the old bytes, not universal requirements for
new records. Ordinary typed JSON needs no canonical digest; actual content identity still hashes bytes.

Change new internal formats and their real ends together; rebuild derived generations. Reject unsupported
old roots before writes and leave their user data intact. An old reader, compatibility shim or export
implementation is not a prerequisite for the new application. Add one only for a separately selected
retained-data use; never silently reinterpret old identities or delete sole copies.

Ciborium 0.2.2 has not been selected as a production dependency. Its
[prior assessment](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
established that it was not a byte-compatible replacement for the old profile. That does not justify
handwritten codecs or block a simpler new format. No new canonical-codec package is needed.

[#253 / PR #322](UNICODE_CASEFOLD_DECISION_2026-10-09.md) already selected exact Unicode 18 C+F full
folding and goldens. Default folding excludes T; Turkic behavior is a distinct profile. No implicit extra
normalization; expanded scalars retain original source ranges. #254 remains the production consumer.
Do not reopen the completed decision, use floating UCD data, or substitute simple regex folding.

## 5. Further donor records: read the relevant section only

| Role | Existing owner / record |
|---|---|
| TOML / globs / matching | #238 typed TOML; #241 explicit globset settings; #246/#247 memchr/aho-corasick; #248 bounded regex-automata. [Detailed pinned source/profile record](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md). Recheck actual source before claiming an engine remains to be adopted. |
| Cargo / Rust syntax | Reuse actual Cargo metadata. #214 retires obsolete map/status policy and the custom partial Rust source analyser; keep private adapter visibility and review the real API. Do not build a pseudo-compiler or controller. |
| Code facts / documents / external corpora | [Implementation](../implementation/README.md) and existing #223–#236/#216 issues retain selected producer/profile boundaries. No speculative universal parser, connector framework or second index. |
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
