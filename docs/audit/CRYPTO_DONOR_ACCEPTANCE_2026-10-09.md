# Crypto donor acceptance — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`
**Issue:** #237
**Assignment:** SA-04 — crypto donor and supply-chain review
**Base SHA inspected:** `5d0435a55db8120d629d14d5167db6d737ea90c2`
**Branch:** `codex/237-canonical-foundation`

**Verdict: ACCEPT.** Three dependency acceptance records below, matching the exact
profiles now present in the tree. No crate version change is proposed or made.

Qualification performed: independent source, checksum, source-commit, license,
feature-gate, transitive-closure and advisory review against a pinned RustSec
advisory database snapshot.

Not performed here: compiled Rust 1.98 execution, Windows/MSVC builds, BLAKE3
test-vector conformance, or any measured reproducibility comparison. Those remain
manager- and main-owned gates. Where a statement below is a design argument rather
than a verified run, it says so.

## Method

Checksums were recomputed locally over the `.crate` archives rather than copied
from the donor register. Source commits were read from each extracted
`.cargo_vcs_info.json`. Feature wiring was read from the upstream
`Cargo.toml.orig` in each extracted crate directory, not from a summary. Closure
was resolved by parsing `Cargo.lock` and following `dependencies` edges with
strict name+version matching. Advisories were searched by content over the local
advisory database, then each hit was checked for version applicability.

---

## 1. `blake3` 1.8.2 — ACCEPT

**Profile accepted:**

```toml
blake3 = { version = "=1.8.2", default-features = false, features = ["std", "pure", "zeroize"] }
```

Present in the tree at `Cargo.toml` line 111, consumed by
`crates/search-contracts`.

| Check | Result |
|---|---|
| .crate SHA-256 | `3888aaa89e4b2a40fca9848e400f6a658a5a3978de7be858e209cafa8be9a4a0` — matches register |
| Source commit | `df610ddc3b93841ffc59a87e3da659a15910eb46` |
| License | `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception`; bundled files `LICENSE_A2`, `LICENSE_A2LLVM`, `LICENSE_CC0` |
| Edition | 2021 |
| `rust-version` | none declared |
| Advisories | none |

Feature gates read from `Cargo.toml.orig`:

- `default = ["std"]`. Disabling default features removes `std`; the profile adds it back explicitly.
- `std = []` — no dependency edges; it enables libstd I/O traits and runtime CPU feature detection.
- `pure = []` — no dependency edges.
- `zeroize = ["dep:zeroize", "arrayvec/zeroize"]` — this is why `arrayvec` gains a `zeroize` edge in the lock.
- `digest`, `traits-preview`, `rayon`, `mmap`, `serde`, `neon`, `wasm32_simd` are all absent. `digest` is optional and gates only trait implementations, so no RustCrypto trait surface reaches `search-contracts`.

**`pure` behaviour, read from `build.rs` rather than assumed.** `is_pure()` reads
`CARGO_FEATURE_PURE`. On x86 targets `main` calls `c_compiler_support()` and then,
under `pure`, selects `build_sse2_sse41_avx2_rust_intrinsics()` rather than
`build_sse2_sse41_avx2_assembly()`. The AVX-512 branch is skipped, and the source
comments that the binary will not include any AVX-512 code.

Three facts stated plainly, each cutting against the feature name:

1. `c_compiler_support()` still constructs a `cc::Build` and still calls
   `is_flag_supported` under `pure`, and `cc = "1.1.12"` remains a build
   dependency. This is not a "no compiler probing" build.
2. Upstream places `pure` below its unstable-feature boundary: features below
   that line are "mainly intended for testing and benchmarking, and they might
   change or disappear at any time without a major version bump." The exact
   `=1.8.2` pin contains this risk and is present.
3. The manager's instruction not to call this a no-compiler-probing build is
   confirmed by the source.

**Reasoning, not measured.** `pure` selects Rust intrinsics instead of Neves'
hand-written x86 assembly and skips the AVX-512 units. I have not counted the
resulting SIMD paths, and I have not established that the resulting binary is
independent of the present MSVC/CC toolchain. `build.emit_rerun_if_env_changed(false)`
does prevent rebuilds when the environment changes, which is a rebuild-stability
property rather than a probing property. Throughput was deliberately not
measured and is not the deciding factor.

## 2. RustCrypto `sha2` 0.10.9 — ACCEPT

**Profile accepted:**

```toml
sha2 = { version = "=0.10.9", default-features = false }
```

Present in the tree at `Cargo.toml` line 112, consumed by `search-contracts`.

| Check | Result |
|---|---|
| .crate SHA-256 | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` — matches register |
| Source commit | `82c36a428f8d6f05f3bfccdedb243e9d1f85359d`, `path_in_vcs` = `sha2` |
| License | `MIT OR Apache-2.0`; bundled `LICENSE-APACHE`, `LICENSE-MIT` |
| Edition | 2018 |
| `rust-version` | none declared |
| Advisories | `RUSTSEC-2021-0100` exists, patched at `>= 0.9.8`; 0.10.9 is unaffected |

`default = ["std"]`, so `default-features = false` disables `std` and with it
`digest/std`. The crate stays usable because `digest 0.10.7` is a mandatory
non-optional dependency and `sha2` is `no_std`-capable by category.

Excluded features, all confirmed absent from the profile:

- `asm = ["sha2-asm"]` — would add the assembly backend. Excluded, keeping the software implementation and avoiding a toolchain-dependent path.
- `oid = ["digest/oid"]` — excluded; upstream notes it bumps MSRV to 1.57.
- `compress`, `force-soft`, `force-soft-compact`, `asm-aarch64`, `loongarch64_asm` — excluded.

`cpufeatures 0.2.17` is a target-gated dependency on x86/x86_64/aarch64, so it
appears in the Windows closure and is unavoidable. It performs runtime CPU
feature detection only and adds no vendor type to the public boundary.

## 3. `syn` 2.0.119 — ACCEPT

**Profile accepted** (in `xtask`, which owns the Rust-aware guard; not a product dependency):

```toml
syn = { version = "=2.0.119", default-features = false, features = ["full", "parsing", "visit", "printing", "clone-impls"] }
```

Present in the tree at `xtask/Cargo.toml` line 16.

| Check | Result |
|---|---|
| .crate SHA-256 | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` |
| Source commit | `3295f9e9841785ac88a5e558c884854d5fb7d67f` |
| Lock checksum | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` — matches |
| License | `MIT OR Apache-2.0`; bundled `LICENSE-APACHE`, `LICENSE-MIT` |
| `rust-version` | `1.71` — below the workspace `1.98`, so no MSRV conflict |
| Edition | 2021 |
| Advisories | none |

`default = ["derive","parsing","printing","clone-impls","proc-macro"]`. Setting
`default-features = false` removes `derive` and `proc-macro`; the accepted set
re-adds `parsing`, `printing` and `clone-impls` and does not re-add either of
those two.

`full` and `visit` supply the syntax tree and visitor surface the guard needs to
match call shapes structurally, which is what distinguishes it from a raw grep
wall. The dependency is not inherited by `search-contracts` or any workspace
product crate.

One clarification required for accuracy. I read the feature wiring in
`Cargo.toml.orig` and in `src/lib.rs`'s `#[cfg(feature = ...)]` gates. That
establishes how `syn` declares and consumes its own features. It does not by
itself establish the final resolved feature set of `proc-macro2` and `quote`,
because Cargo feature unification across the workspace is decided by the resolver,
and `xtask` also depends on `serde_json`, which pulls `serde` with derive. I did
not run `cargo tree -e features`, so I am not claiming that the proc-macro
dependency closure is eliminated. The verified statement is narrower: `syn 2.0.119`
with `default-features = false` and this feature list does not request
`proc-macro2/proc-macro` or `quote/proc-macro` through its own dependency edges.

---

## Transitive closure

Resolved from `Cargo.lock` with strict name+version edge following, starting at
blake3, sha2, syn, toml and serde_json. 42 packages.

| Layer | Packages |
|---|---|
| direct | `blake3 1.8.2`, `sha2 0.10.9`, `syn 2.0.119`, `toml 0.8.23`, `serde_json 1.0.151` |
| crypto | `digest 0.10.7`, `crypto-common 0.1.7`, `block-buffer 0.10.4`, `generic-array 0.14.7`, `typenum 1.20.1`, `constant_time_eq 0.3.1`, `arrayvec 0.7.6`, `arrayref 0.3.9`, `cpufeatures 0.2.17`, `rand_core 0.6.4`, `zeroize 1.9.0` |
| build | `cc 1.4.5`, `shlex 2.0.1`, `find-msvc-tools 0.1.12` |
| toml/serde | `toml_edit 0.22.27`, `toml_datetime 0.6.11`, `toml_write 0.1.2`, `serde_spanned 0.6.9`, `winnow 0.7.15`, `serde_core 1.0.229`, `serde_derive 1.0.229`, `indexmap 2.14.2`, `hashbrown 0.17.1`, `equivalent 1.0.2`, `memchr 2.8.3`, `itoa 1.0.18`, `zmij 1.0.23` |
| platform | `cfg-if 1.0.4`, `libc 0.2.189`, `getrandom 0.2.17`, `wasi 0.11.1+wasi-snapshot-preview1`, `version_check 0.9.5`, `proc-macro2 1.0.107`, `quote 1.0.47`, `unicode-ident 1.0.24` |

Two resolution facts worth recording, because both are easy to misread as scope creep:

- `syn@3.0.5` also exists in the lock, reached via `serde_derive` and `futures-macro`. It is a separate resolution. `xtask`'s only `syn` edge is `syn 2.0.119`, so the accepted profile is unaffected by the 3.x line.
- `serde_derive` is pulled in because `serde_json` enables `serde` with derive, and `serde_json` is a pre-existing `xtask` dependency. `syn 2.0.119` does not add it; `syn`'s own `build = false`.

Nothing in the closure adds a second search engine, codec, network client or scheduler. `zmij` is a `serde_json` dependency, not a new data path.

## Advisory screening

**Snapshot:** advisory database commit
`7eebec69c352c7191b1f13eb95dd510eeca5d1de`, dated `2026-10-09T10:12:02+02:00`.
Screening ran against this pinned snapshot, not a live refresh.

Content search over all 42 closure packages returned eight candidate hits. Each
was checked for version applicability:

| Crate | Advisory | Applicability | Disposition |
|---|---|---|---|
| `blake3 1.8.2` | none | — | clean |
| `sha2 0.10.9` | `RUSTSEC-2021-0100` | patched `>= 0.9.8` | unaffected |
| `syn 2.0.119` | none | — | clean |
| `arrayref 0.3.9` | `RUSTSEC-2026-0260` | malicious 0.3.10 only; `unaffected = ["<= 0.3.9"]` | explicitly unaffected |
| `generic-array 0.14.7` | `RUSTSEC-2020-0146` | patched ranges include `>= 0.13.3` | patched |
| `hashbrown 0.17.1` | `RUSTSEC-2024-0402` | affected function pinned to `=0.15.0`; patched `>= 0.15.1` | patched |
| `rand_core 0.6.4` | `RUSTSEC-2021-0023`, `RUSTSEC-2019-0035` | affected functions `< 0.6.2` and `< 0.4.2` | unaffected |
| `shlex 2.0.1` | `RUSTSEC-2024-0006` | patched `>= 1.3.0` | patched |
| `serde` | `RUSTSEC-2020-0071` | record's `package` field is `time`, not `serde` | incidental text match, not a serde advisory |

Every advisory in the closure is patched, explicitly unaffected, or
function-scoped to a version far below the pinned one.

One item deserves recording rather than being passed over. `arrayref` was the
subject of a supply-chain compromise on 2026-08-20
(`RUSTSEC-2026-0260`): version 0.3.10 was published with a malicious
`proc-macro1` dependency, was live for roughly 86 minutes, and was downloaded
2,285 times. This is the strongest single argument for exact pinning rather than
caret ranges in a package touching digest computation, and the direct reason the
lock must not float `arrayref` above `0.3.9`.

## Upstream references

- [blake3 repository](https://github.com/BLAKE3-team/BLAKE3), [1.8.2 metadata](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.2/Cargo.toml)
- [RustCrypto hashes](https://github.com/RustCrypto/hashes), [sha2-v0.10.9 metadata](https://github.com/RustCrypto/hashes/blob/sha2-v0.10.9/sha2/Cargo.toml)
- [syn repository](https://github.com/dtolnay/syn)
- [RustSec advisory database](https://github.com/RustSec/advisory-db)

## Blockers and unknowns

No acceptance blocker was found. The following remain open and are owned by main,
not by this report:

1. **Rust 1.98 compile and strict Clippy for `search-contracts` and `xtask`, all targets.** Not executed. `blake3` and `sha2` declare no `rust-version`, so compatibility is an inference from edition (2021 and 2018), not a compiled fact.
2. **Windows/MSVC build of the `pure` profile.** `pure` still runs `c_compiler_support()`, which under MSVC probes `/arch:AVX512`. A machine where MSVC is absent or rejects that flag yields `NoCompiler`, and `pure` then selects the Rust intrinsics path. That fallback is exercised in source but unverified here.
3. **Reproducibility.** The pure-versus-optimized comparison is argued from build-surface reasoning, not measured. Any reproducibility claim needs a build matrix, out of scope here.
4. **BLAKE3 vector conformance.** Nothing here proves output parity against official test vectors. That is a fixture and golden-corpus task, not an acceptance one.
5. **Resolved `proc-macro2`/`quote` features.** Not measured. See the clarification in the `syn` record.

## Disposition

`ACCEPT` for manifest modification, on the three exact profiles above, at the base
SHA inspected. No crate version change is proposed. A verified advisory or API
contradiction would halt merge; none was found.

---

# Tooling-only donor records added to SA-04 (guard scope, not product scope)

These two records cover the #237 guard tooling only. They do not qualify any
product code path, and neither is evidence for or against the #238 product-path
cutover, which is a separate owner issue.

# 4. `proc-macro2` 1.0.107 — ACCEPT (tooling-only)

**Profile:** `default-features = false`, as a direct `xtask` dependency for token
bounding ahead of the `syn` AST parse.

| Check | Result |
| --- | --- |
| Archive SHA-256 | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` — recomputed locally, matches the `Cargo.lock` checksum |
| Lock state | unchanged release; identical entry already present on `origin/main` |
| Source commit | `ed8a5497669cd63db33bf24646f261b012bbbc4a` |
| License | `MIT OR Apache-2.0` |
| MSRV | `1.71` — below workspace `1.98` |
| Edition | 2021 |
| Normal closure | `unicode-ident` only |
| Advisories | none for this crate |

`default = ["proc-macro"]` is the load-bearing part of this profile. Disabling it
decouples the token layer from the compiler proc-macro server, which is what makes
the crate usable from a plain binary tool. The consequence is that `TokenStream::from_str`
at `src/lib.rs:264` routes to the pure-Rust fallback at `fallback.rs:83`
(`from_str_checked`), which strips an optional BOM and calls `parse::token_stream`.
The `wrap_proc_macro` wrapper at `src/lib.rs:1244-1258`, which catches panics and
maps rustc lex errors, is not compiled.

# 5. `toml` 1.1.7+spec-1.1.0 — ACCEPT (tooling-only guard manifest parser)

**Profile:** aliased in `xtask/Cargo.toml` so the existing tooling parser is not
replaced:

```toml
toml = "0.8"
manifest-toml = { package = "toml", version = "=1.1.7", default-features = false,
                   features = ["std", "serde", "parse"] }
```

| Check | Result |
| --- | --- |
| Archive SHA-256 | `7874cd34dd99040fc24e30923aa64a60a81ff3ddfe2b8129ccc84ba24949deed` — matches lock checksum |
| Source commit | `f72038c2508112dfaaedb227a511f91bf9b2015c` (`path_in_vcs = "crates/toml"`) |
| MSRV | `1.85` — below workspace `1.98` |
| Edition | 2024 |
| License | `MIT OR Apache-2.0` |
| `build` | `false`, no build script |
| Advisories | none |

Excluded features and why: `display` (omits `toml_writer`), `unbounded` (upstream
documents it as disabling recursion protection), `debug` (omits `anstream`/`anstyle`,
keeping `RUSTSEC-2024-0404` out of this profile), `preserve_order`/`fast_hash`
(omit `indexmap` and `foldhash`).

# Active vs inactive closure

`Cargo.lock` records dependency edges, not only active ones, so lock presence alone
cannot distinguish an optional inactive dependency from an active one. The
distinction below is the feature-activation reading of the manifest wiring; I did
not run `cargo tree`, so it is an inference from the manifest rather than an
observed solver graph.

| Crate | Lock version | MSRV | State |
| --- | --- | --- | --- |
| `toml` | 1.1.7+spec-1.1.0 | 1.85 | active root |
| `toml_parser` | 1.1.4+spec-1.1.0 | 1.85 | active via `parse`; pinned down from 1.1.5 |
| `winnow` | 1.0.4 | 1.65.0 | active via `parse`; required dependency |
| `serde_spanned` | 1.1.2 | 1.85 | active; not optional |
| `toml_datetime` | 1.1.2+spec-1.1.0 | 1.85 | active; not optional |
| `serde_core` | 1.0.229 | 1.56 | active via `serde` |
| `toml_writer` | 1.1.3+spec-1.1.0 | — | in lock, inactive; gated on `display`, which is not enabled |

Every archive SHA-256 above was recomputed locally against the `Cargo.lock`
checksum for its version, and each matched.

`serde_spanned` and `toml_datetime` are non-optional, so they enter the closure even
with all features off; the minimum active closure is 6 crates, not 1.
`toml_writer` is listed in the lock and in `toml`'s `dependencies` array, so absent
is not claimed. The confirming observation would be `cargo tree -i toml_writer`,
which is a separate step from this report.

Manager verification on 2026-10-09: the locked `cargo tree -p xtask -e normal,build`
and inverse `toml_writer` query confirm that `toml_writer` is inactive for this
target. The inverse feature tree for `proc-macro2` confirms no `proc-macro`
feature in the guard's xtask closure. This supplements the independent source
review above with the observed solver graph; other workspace consumers may
activate different features in their own build closures.

# Parser API and depth bound

The guard must use the document deserializer, not the value one. Both are verified
in the 1.1.7 source:

- `impl FromStr for Value` at `src/value.rs:395` routes to `ValueDeserializer::parse`, which calls `DeValue::parse` and parses a single TOML value. Its doctest is additionally gated on `display`, so it is not compiled under the accepted feature set.
- `toml::de::from_str` at `src/de/mod.rs:72` routes to `Deserializer::parse`, which calls `DeTable::parse` and parses a table, i.e. a document. This is the correct path for a `Cargo.toml`.

This is an API-shape difference between parsing a value and parsing a document, not
a donor defect. It is recorded here for #238 to carry forward.

Recursion is bounded internally with no caller configuration.
`RecursionGuard::new(&mut receiver, LIMIT)` at `src/de/parser/mod.rs:37` with
`const LIMIT: u32 = 80;` at `src/de/parser/mod.rs:71`. An earlier requirement in
this report that a caller set `max_depth` explicitly is withdrawn: there is no
public numeric setter, and the bound of 80 is built in.
`src/de/parser/key.rs:67` applies the same `LIMIT` to key-path length separately.
