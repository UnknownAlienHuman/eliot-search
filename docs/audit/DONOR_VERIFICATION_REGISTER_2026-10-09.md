# Donor verification register — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Reviewed main:** `e358d3c06d211b269efd501cd8d4002a260ba0a5`  
**Purpose:** distinguish accepted, rejected and still-unverified donors before one manager delegates work to subagents.  
**Qualification performed:** static source/documentation review only; no exact-head Cargo, Windows or Qdrant run is claimed.

## Status vocabulary

| Status | Meaning |
|---|---|
| `ACCEPTED_EXACT` | Exact release/source/profile is selected for the named narrow role. |
| `REJECTED_PRODUCTION` | May be used as a test oracle or research source, but not as product authority. |
| `SELECTED_REVERIFY` | Direction is sound, but the assigned slice must recheck exact release, checksum, MSRV, features, advisories and API before changing `Cargo.lock`. |
| `DECISION_REQUIRED` | No implementation starts until the named research task freezes one choice and golden corpus. |

Popularity, benchmark marketing or a dependency already appearing in `Cargo.lock` is never sufficient acceptance.

## Normative product boundary

Read before evaluating any donor:

- [Root agent rules](../../AGENTS.md)
- [Architecture entrypoint](../architecture/README.md)
- [Normative Architecture Part I](../architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md)
- [ADR-0005 — standalone Search and controller boundary](../adr/0005-standalone-search-product-and-controller-boundary.md)
- [ADR-0006 — agent-analysis product scope](../adr/0006-agent-analysis-framework-product-scope.md)
- [Current launch packet](./WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md)

A donor owns a mechanism. It never inherits ELIOT root, source, access, currentness, receipt, completeness, readiness or lifecycle authority.

## Wave-1 canonical/digest foundation

### Existing `search-contracts` canonical codec — `ACCEPTED_EXACT`

Keep the existing closed `CanonicalValue`, bounded JSON/CBOR encoders and strict decode/re-encode checks. Do **not** create `search-canonical` and do not replace the closed value vocabulary with arbitrary Serde values.

Current source:

- [`canonical.rs`](../../crates/search-contracts/src/canonical.rs)
- [`ids.rs`](../../crates/search-contracts/src/ids.rs)
- [conformance fixtures](../../crates/search-contracts/tests/conformance.rs)

Normative standard:

- [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html)

Important profile correction: the current encoder orders map keys by encoded-key length and then bytewise value. That is the RFC 8949 **length-first deterministic ordering** profile described in §4.2.3, not the §4.2.1 Core Deterministic ordering. Existing accepted bytes must be frozen. Changing ordering requires a new canonical profile and migration; it is not a cleanup inside #237.

### `blake3` 1.8.2 — `ACCEPTED_EXACT`

Exact source and metadata:

- [official `blake3` 1.8.2 `Cargo.toml`](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.2/Cargo.toml)
- [official implementation repository](https://github.com/BLAKE3-team/BLAKE3)
- lockfile checksum: `3888aaa89e4b2a40fca9848e400f6a658a5a3978de7be858e209cafa8be9a4a0`
- license: `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception`

Accepted narrow profile for #237:

```toml
blake3 = { version = "=1.8.2", default-features = false, features = ["std", "pure", "zeroize"] }
```

Risks that must remain explicit:

- upstream documents `pure` below its unstable/testing-feature boundary;
- exact pinning is therefore mandatory;
- `pure` is selected to avoid assembly/C implementations in this authority package, not because it is a generally stable donor feature;
- no `rayon`, `mmap`, `serde`, `traits-preview`, platform SIMD opt-in or runtime file helper belongs in `search-contracts`;
- the manager must prove Rust 1.98 and target compatibility before merge.

The crate performs the named algorithm only. ELIOT owns domain/profile separation, input ceilings, operation schemas and typed results.

### RustCrypto `sha2` 0.10.9 — `ACCEPTED_EXACT`

Exact source and metadata:

- [official `sha2-v0.10.9` `Cargo.toml`](https://github.com/RustCrypto/hashes/blob/sha2-v0.10.9/sha2/Cargo.toml)
- [official RustCrypto hashes repository](https://github.com/RustCrypto/hashes)
- lockfile checksum: `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283`
- license: `MIT OR Apache-2.0`

Accepted narrow profile:

```toml
sha2 = { version = "=0.10.9", default-features = false }
```

Do not enable `asm`, `oid`, exposed compression APIs or another SHA implementation. Existing copied SHA loops are owner migrations; #237 adds the shared compute boundary and guard rather than rewriting every consumer in one branch.

### Ciborium 0.2.2 — `REJECTED_PRODUCTION`

Evidence:

- [exact 0.2.2 canonical value source](https://github.com/enarx/ciborium/blob/v0.2.2/ciborium/src/value/canonical.rs)
- [0.2.2 package documentation](https://docs.rs/crate/ciborium/0.2.2)

The exact stable release provides canonical comparison/`CanonicalValue`, but not a public `canonical_into_writer` production API. Its ordering code also serializes dynamic values for comparison. It may serve as an independently pinned differential oracle in tests; it must not become the product codec or leak donor `Value` types across ELIOT boundaries.

## Unicode full case folding — `DECISION_REQUIRED` under #253

Normative primary data for the current review date:

- [Unicode 18.0.0 `CaseFolding.txt`](https://www.unicode.org/Public/18.0.0/ucd/CaseFolding.txt)
- [UAX #44 — Unicode Character Database](https://www.unicode.org/reports/tr44/)
- [UAX #15 — normalization](https://www.unicode.org/reports/tr15/)
- [UAX #31 — identifiers and syntax](https://www.unicode.org/reports/tr31/)
- [Issue #253](https://github.com/UnknownAlienHuman/eliot-search/issues/253)

Facts that the decision must preserve:

- full folding uses `C + F` mappings and may expand one scalar into several;
- default folding excludes `T`; Turkic behavior is a distinct profile;
- case folding does not preserve normalization by itself;
- original source ranges must remain exact even when normalized terms expand;
- floating `latest` data is forbidden in implementation and fixtures.

Candidate assessment:

| Candidate | Current assessment |
|---|---|
| Generated checked-in table from exact Unicode 18.0.0 data | Preferred baseline candidate: smallest exact transformation surface, explicit data provenance and suitable for per-scalar range mapping. Generator, checksum, license header and differential fixtures must be retained. |
| `focaccia` | Useful oracle/candidate: current releases expose full/Turkic comparisons and a Unicode version, but the public API is comparison-oriented. Do not select it until exact release source proves a deterministic emitted-fold/range-mapping API suitable for indexing. |
| ICU4X case mapping | Fallback only if the small generated-table path cannot satisfy exact behavior. Its broader data/dependency surface is not justified merely to obtain folding. |
| simple-fold-only crates / regex case folding | Rejected for `prose_words@1`: they omit multi-scalar `F` mappings such as `ß → ss`, and regex libraries intentionally use simple folding. |

#253 remains a real decision gate. A subagent may recommend the generated-table option, but the manager must publish the exact data version, generator, checksum, default/Turkic policy, normalization order and byte-for-byte goldens before #254 starts.

## Wave-2 donor families

These selections are directionally sound and preferable to the current handwritten engines, but each implementation issue must revalidate the exact release at its post-#237 base. They are **not** additional Wave-1 coding scope.

| Slice | Selected mechanism | Status | Exact task / primary entrypoint |
|---|---|---|---|
| Typed TOML | `toml` + Serde typed visitor + `serde_path_to_error` | `SELECTED_REVERIFY` | [#238](https://github.com/UnknownAlienHuman/eliot-search/issues/238), [toml-rs](https://github.com/toml-rs/toml) |
| Admission globs | `globset` over ELIOT canonical relative bytes | `SELECTED_REVERIFY` | [#241](https://github.com/UnknownAlienHuman/eliot-search/issues/241), [globset source](https://github.com/BurntSushi/ripgrep/tree/master/crates/globset) |
| Single/multi literal matching | `memchr::memmem` + `aho-corasick` | `SELECTED_REVERIFY` | [#246](https://github.com/UnknownAlienHuman/eliot-search/issues/246), [#247](https://github.com/UnknownAlienHuman/eliot-search/issues/247), [memchr](https://github.com/BurntSushi/memchr), [aho-corasick](https://github.com/BurntSushi/aho-corasick) |
| Bounded regex | `regex-automata` closed profile | `SELECTED_REVERIFY` | [#248](https://github.com/UnknownAlienHuman/eliot-search/issues/248), [regex-automata docs](https://docs.rs/regex-automata/) |
| Cargo graph | `cargo_metadata` | `SELECTED_REVERIFY` | [#250](https://github.com/UnknownAlienHuman/eliot-search/issues/250), [cargo_metadata](https://github.com/oli-obk/cargo_metadata) |
| Rust syntax lint | `syn::parse_file` + narrow visitor | `SELECTED_REVERIFY` | [#251](https://github.com/UnknownAlienHuman/eliot-search/issues/251), [syn](https://github.com/dtolnay/syn) |
| Code identifiers | `unicode-ident` + `unicode-normalization` | `SELECTED_REVERIFY` | [#252](https://github.com/UnknownAlienHuman/eliot-search/issues/252), [unicode-ident](https://github.com/dtolnay/unicode-ident), [unicode-normalization](https://github.com/unicode-rs/unicode-normalization) |
| Prose segmentation | `unicode-segmentation` + #253 fold profile | blocked | [#254](https://github.com/UnknownAlienHuman/eliot-search/issues/254), [unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation) |
| Markdown/JATS | `pulldown-cmark` + `quick-xml` behind separate profiles | `SELECTED_REVERIFY` | [#226](https://github.com/UnknownAlienHuman/eliot-search/issues/226), [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark), [quick-xml](https://github.com/tafia/quick-xml) |
| Git plumbing | narrow `gitoxide`/`gix` profile | `SELECTED_REVERIFY` | [#129](https://github.com/UnknownAlienHuman/eliot-search/issues/129), [gitoxide](https://github.com/GitoxideLabs/gitoxide) |
| Structural facts | Tree-sitter as syntax producer, not semantic authority | `SELECTED_REVERIFY` | [#223](https://github.com/UnknownAlienHuman/eliot-search/issues/223), [Tree-sitter](https://github.com/tree-sitter/tree-sitter) |
| Precise imported facts | official SCIP stream/profile | `SELECTED_REVERIFY` | [#224](https://github.com/UnknownAlienHuman/eliot-search/issues/224), [SCIP](https://github.com/sourcegraph/scip) |

## Mandatory donor acceptance record

Before a manager changes a manifest or lockfile, the responsible subagent returns:

```text
exact crate/package and role
exact version, tag and source commit
registry checksum
license and bundled-data terms
MSRV against Rust 1.98
normal/build/dev transitive closure
explicit enabled and disabled features
advisories and maintenance state
API/source facts required by the issue
resource/cancellation limitations
public-type containment boundary
rejected alternatives and concrete reason
fixture/differential oracle
ACCEPT / REJECT / BLOCK
```

The manager independently checks the load-bearing source lines. A subagent report, Context7 summary, README claim or generated benchmark cannot authorize adoption by itself.

## Conclusion

The immediate #237 donors are now sufficiently verified for a controlled implementation start, with the `blake3::pure` risk and RFC 8949 ordering profile explicitly recorded. The wider donor program is well selected but deliberately remains `SELECTED_REVERIFY` per slice. Claiming that every donor is already fully qualified would be false; the single-manager packet makes exact donor revalidation a precondition rather than allowing agents to improvise.