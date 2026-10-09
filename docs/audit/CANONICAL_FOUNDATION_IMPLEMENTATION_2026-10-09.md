# Canonical foundation implementation — 2026-10-09

Owner: [#237](https://github.com/UnknownAlienHuman/eliot-search/issues/237).
Initial launch source: `5d0435a55db8120d629d14d5167db6d737ea90c2`.
Accepted base after the coordinator's documentation-only update:
`25616fa4d422815ce9402c4d703d4b63129fe9b3`.
The source-gate prerequisite #317/#318 is merged at
`776c7f3230406d918b9805a5b5e079a4baa84ea0`; this is the implementation base.
One manager worktree, branch `codex/237-canonical-foundation-delivery`. The maintainer's
current request permits code-writing OpenCode Go1 Step 5 preview subagents;
non-overlapping file ownership replaces the packet's read-only delegation rule.
The manager alone integrates dependencies, commits, pushes and merges.

## Code understanding proof

The shared owner is `search-contracts`, never a new canonical crate. Exact source
anchors identify `to_canonical_cbor`,
`domain_separated_preimage`, `VersionedContentDigest` and algorithm digest wrappers.
The launch encoder validates the tree, grows a vector, then checks output size;
the legacy domain helper retains another complete preimage. This is the causal
allocation defect, independent of any Qdrant or daemon changes.

The new path is closed value validation → checked encoder sink → vector or
private algorithm hasher → complete 32-byte ELIOT digest. No storage, process,
network, authority receipt or operation journal is added. Codec, IDs and compute
APIs are reviewed together; semantic owner migrations remain separate issues.
The guard owns source classification only. It cannot prove runtime dataflow or
qualify an operation's semantic preimage.

Required verifiers are the locked Rust 1.98 check and strict Clippy for
`search-contracts` and `xtask`, all targets, plus the current-tree guard command.
Golden and guard fixtures are compiled with the source. Full test execution and
native/Qdrant/release qualification remain outside this implementation pass.

## Byte and digest profile

`eliot.cbor.length-first.v1` preserves existing JSON and RFC 8949 §4.2.3 CBOR
bytes, shortest lengths/integers, duplicate rejection and strict re-encoding.
It does not change the map ordering to RFC 8949 §4.2.1.

New digest domains are bounded to 128 ASCII bytes and use
`eliot/cbor/<owner-schema>/vN` or `eliot/raw/<owner-schema>/vN`. Nonempty lowercase
segments permit internal single hyphens; the revision is a nonzero `u32` decimal
without leading zeroes. Prefixes prevent raw and CBOR helper interchange. The
owner must freeze its schema, domain and field revisions in golden fixtures.
Parsing a domain is validation, not authority issuance.

Every preimage is exactly `domain UTF-8 || NUL || payload`. Canonical payloads
use the sole CBOR encoder. Raw payloads remain exact bytes. `DigestInputLimit`
counts the domain and separator, is nonzero and cannot exceed 8 MiB. A failed
encoder yields no digest. No complete canonical payload/preimage allocation is
needed by compute helpers. Decoded bytes remain explicitly decode-only; the
compatibility constructor does not assert that a real algorithm was executed.

Secret-derived `HandleTokenDigest` has a separate constant-redacted `Debug` and
`Display` surface. Explicit 32-byte restore/access and ordering are retained;
ordinary formatting no longer serializes the bytes. Its algorithm/profile
construction remains semantic owner work under #282.

The typed exception ledger records legacy algorithms as legacy. Replacing a
mixer with real cryptography requires its owner's new profile/generation or
rebuild decision, never relabelling old bytes.

## Exact donor record

The manager read the registry source and original Cargo metadata and independently
hashed the cached registry archives. The selected archives match `Cargo.lock`:

| Private donor | Source commit | Registry SHA-256 | License | Direct profile |
|---|---|---|---|---|
| BLAKE3 1.8.2 | `df610ddc3b93841ffc59a87e3da659a15910eb46` | `3888aaa89e4b2a40fca9848e400f6a658a5a3978de7be858e209cafa8be9a4a0` | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | default off; std, pure, zeroize |
| SHA-2 0.10.9 | `82c36a428f8d6f05f3bfccdedb243e9d1f85359d` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` | MIT OR Apache-2.0 | default off |
| syn 2.0.119, xtask AST only | `3295f9e9841785ac88a5e558c884854d5fb7d67f` | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` | MIT OR Apache-2.0 | default off; full, parsing, visit, printing, clone-impls |

`syn` declares Rust 1.71. The two algorithm manifests do not declare an MSRV;
Rust 1.98 target compatibility must therefore be demonstrated by this branch's
source gates rather than inferred from missing metadata. Selected closure
metadata includes `zeroize` 1.9.0 (Rust 1.85), `cc` 1.4.5 (1.65),
`constant_time_eq` 0.3.1 (1.66), `proc-macro2` 1.0.107, `quote` 1.0.47 and
`unicode-ident` 1.0.24 (1.71). Registry resolution preserved all existing versions.

The BLAKE3 `pure` feature is explicitly upstream testing/unstable material.
Exact pinning limits that compatibility risk. Source `build.rs` selects Rust
SSE2/SSE4.1/AVX2 intrinsics and omits AVX-512 implementations with this profile;
the `cc` dependency and compiler capability probing remain. This is not a claim
that every build script or compiler probe is absent. The optimized alternative
adds assembly/C selection to this build boundary. No mmap, rayon, serde,
traits-preview, explicit NEON or wasm SIMD feature is enabled here.

SHA-2 asm, OID and exposed compression features are not enabled by this owner.
`syn` is tooling-only: AST inspection cannot resolve types, expand arbitrary
macros or prove dataflow. Those limitations must remain visible in the guard.

Primary sources: [BLAKE3 1.8.2](https://github.com/BLAKE3-team/BLAKE3/tree/1.8.2),
[SHA-2 0.10.9](https://github.com/RustCrypto/hashes/tree/sha2-v0.10.9/sha2),
[syn registry release](https://crates.io/crates/syn/2.0.119).
Independent advisory/source acceptance is recorded in
[SA-04's report](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md).

The bounded guard also pins tooling-only `proc-macro2` 1.0.107 with defaults off
and `manifest-toml` (package `toml`) 1.1.7 with only `std`, `serde`, `parse`.
The old tooling TOML parser remains unchanged. The modern document parser accepts
the existing Cargo 1.98 manifest syntax and retains its built-in recursion bound
of 80; `Value::from_str` parses a value and is deliberately not used here.
The lock pins `toml_parser` 1.1.4, `serde_spanned` 1.1.2,
`toml_datetime` 1.1.2 and `winnow` 1.0.4. Exact hashes, licenses, source commits
and active/inactive closure are recorded in the same donor report. This tooling
decision does not adopt a product parser or qualify #238.

The manager independently checked matching RustSec TOML records in snapshot
`7eebec69c352c7191b1f13eb95dd510eeca5d1de` (2026-10-09):
`arrayref` 0.3.9 is explicitly unaffected by RUSTSEC-2026-0260;
`generic-array` 0.14.7 satisfies the patched range of RUSTSEC-2020-0146;
SHA-2 0.10.9 satisfies RUSTSEC-2021-0100's patched range;
`shlex` 2.0.1 satisfies RUSTSEC-2024-0006's patched range. No matching record
was found for the other selected algorithm/AST closure names. This describes
that exact advisory snapshot, not absence of every possible vulnerability or a
full workspace supply-chain audit.

## Evidence status

The retained daemon SHA implementation is owned by
[#315](https://github.com/UnknownAlienHuman/eliot-search/issues/315); materializer
framed digests are owned by [#316](https://github.com/UnknownAlienHuman/eliot-search/issues/316).
Source-admission copied SHA/preimage migration is owned by
[#320](https://github.com/UnknownAlienHuman/eliot-search/issues/320), and retained
daemon BLAKE3/CNG/preimages and development fingerprints by
[#321](https://github.com/UnknownAlienHuman/eliot-search/issues/321).
These migrations are intentionally outside #237. Legacy profile bytes cannot be
relabeled as new domain/schema identities.

The final PR records the exact checked revision, platform, toolchain, scoped
commands and independent-review outcomes. Historical source gates are not
inherited by this branch; full product/native/Qdrant qualification is separate.
The narrow revision-store residency fixture uses the explicit stored-byte
constructor after digest fields became private. Its consumer check passes;
strict consumer Clippy has nine existing diagnostics tracked under #270, and
is not included in the passing contracts/guard claim.
