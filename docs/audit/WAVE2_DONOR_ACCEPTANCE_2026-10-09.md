# Wave 2 donor acceptance register — 2026-10-09

**Preparation base:** `5d0435a55db8120d629d14d5167db6d737ea90c2`  
**Coding base:** exact post-`#237` main, to be published by `#97`  
**Purpose:** freeze mechanisms and reject unsuitable defaults before one manager changes manifests or `Cargo.lock`.  
**Evidence boundary:** primary source/static review only; implementation-time checksum/advisory/locked resolution and Rust 1.98 gates remain mandatory.

## Status vocabulary

| Status | Meaning |
|---|---|
| `PORT_CURRENT` | Use current ELIOT source as the implementation base; no external framework required. |
| `PORT_DONOR_SNAPSHOT` | Port only named model/fixtures from a closed commit; never merge its branch wholesale. |
| `SELECTED_EXACT_SOURCE` | Exact release/tag/API is selected; registry checksum and resolved closure are re-read immediately before lockfile change. |
| `SELECTED_WITH_CORRECTION` | Mechanism is selected, but the existing issue contains a concrete API/feature/boundary error corrected here. |
| `REJECTED_PRODUCTION` | May be a test oracle or research source only. |
| `BLOCKED_DECISION` | No code until another named decision closes. |

A version number alone is not acceptance. The manager records registry checksum, license, MSRV, advisories, normal/build/dev closure and enabled/disabled features in the PR.

# 1. Slices without external production donors

## `#258` shared S9.5/S10.3 contract — `PORT_DONOR_SNAPSHOT`

Use current `search-contracts` plus only named schema/filter fixtures from:

```text
#209 head e5c14cde20292fec59e5cabcc921eda3ae5aaa67
#200 head 615d64f70d8b953cc585990659b865cfe250092b
```

Retain:

- exact provider-neutral field/index/vector/eligibility tables;
- generation-local epoch rules;
- conformance fixtures.

Reject:

- duplicated planner/bridge tables;
- Qdrant SDK types in `search-contracts`;
- package-local canonical writers/digests;
- branch topology and any current-authority assumptions.

## `#256` point identity — `PORT_DONOR_SNAPSHOT`

Use current package plus named files from `#207` head `f0ac8a1470336475f28e3fb30610947c83bbb6d1`:

```text
key.rs
collision.rs
uuid.rs
tests.rs
```

Retain the eight-field S11 key, full-digest collision refusal and UUID fixtures. Reject donor `canonical.rs`, direct generic BLAKE3 ownership and mutable compatibility state. All bytes/digests come through accepted `#237` APIs.

## `#257` UnitSet v3 — `PORT_CURRENT`

Current `search-unitizer` already owns manifest, layout, spans and deterministic profile behavior. Extend it into one verified typed UnitSet. Do not introduce a generic document model, database or another unitizer framework.

## `#266` root owner — `PORT_CURRENT`

Current `search-runtime-owner`, daemon owner composition and control/quarantine source contain the necessary mechanics. Copy only mature state-machine invariants already selected in the audit; do not import a service framework or second owner catalog.

The 2026-10-10 #266 continuation verified a **retained-input source gap** in
these current owners: `DataRootRequest` hashed but discarded the canonical value,
the quarantine marker is a fixed presence payload, and cutover records retain
root/incarnation/snapshot chains without an exact invocation/input record. The
request now retains its immutable canonical value/kind and initialization checks
the full native input before acquisition. This does not qualify general durable
named recovery. Its exact operation record and reconciliation remain open in
#266/#268/#269; do not substitute counts, PID, digests or catalog verification for
that evidence. No external recovery framework is selected by this donor row.

The next producer prerequisite reuses `search-revision-store/src/immutable_object.rs`
hard-link no-clobber publication as a primitive only. It retains full original
input and the actual ACTIVE owner record in shared canonical CBOR v2 at the
existing catalog-quarantine locators for four one-shot consumers. It does not
copy revision-object lifecycle, select a dependency, decode evidence into owner
authority or qualify general named recovery. Existing staging/final evidence is
never reused or removed during arm/error/Drop. Cleanup is a distinct owned
completion after actual release; its later crash and native unlink phases are
not inferred from the donor's immutable-object acceptance.

Named catalog evidence inspection also reuses current `DataRootGuard::with_existing_lock`,
`NativeLayoutPins`, exact existing owner inspection, canonical parsing and the
original request's shared canonical digest/domain. It selects no external framework
and grants no ordinary store/recovery-completion authority. An exact-source search
found no existing disposition-by-handle unlink donor in the daemon/library tree;
#347 leaves that native mechanism, sharing/DELETE semantics and late cleanup
qualification for separate review. Current path-based cleanup is not relabelled
as atomic original-object deletion or power-loss durability.

## `#235` provider client — `PORT_CURRENT`

Extract the existing typed CLI implementation:

```text
bins/eliot-search/src/provider_client/typed.rs
typed/io.rs
typed/local.rs
typed/native.rs
typed/state.rs
```

Reject `provider_client/core.rs` as the canonical public client: it remains legacy/loopback harness material until deleted or isolated. No external networking/client framework is required.

# 2. Typed TOML — `#238`

## Selected exact source

```text
toml crate:             1.1.7+spec-1.1.0
git tag:                toml-v1.1.7
source package MSRV:    1.85
license:                MIT OR Apache-2.0

serde:                  1.0.228
serde MSRV:             1.56
license:                MIT OR Apache-2.0

serde_path_to_error:    0.1.20
MSRV:                   1.61
license:                MIT OR Apache-2.0
```

Primary source:

- <https://github.com/toml-rs/toml/blob/toml-v1.1.7/crates/toml/Cargo.toml>
- <https://github.com/toml-rs/toml/blob/toml-v1.1.7/Cargo.toml>
- <https://github.com/serde-rs/serde/blob/v1.0.228/serde/Cargo.toml>
- <https://github.com/dtolnay/path-to-error/blob/0.1.20/Cargo.toml>

## Accepted feature profile

```toml
toml = { version = "=1.1.7+spec-1.1.0", default-features = false, features = ["std", "serde", "parse"] }
serde = { version = "=1.0.228", default-features = false, features = ["std"] }
serde_path_to_error = "=0.1.20"
```

`serde/derive` is not selected by default. Prefer a custom bounded `Visitor`/`DeserializeSeed`; enable derive only if the manager proves a small closed envelope materially reduces code without retaining a dynamic tree or widening the dependency boundary.

Explicitly absent:

```text
toml display
preserve_order
fast_hash
debug
unbounded
toml::Value product state
toml_edit
Figment/config-rs layering
```

The donor owns TOML syntax only. ELIOT owns captured input bounds, closed fields, precedence, secrets, canonical effective snapshot and reconfiguration semantics.

# 3. Admission globs — `#241`

## Selected exact source

```text
globset:             0.4.20
reviewed commit:     3fce3b5bb0236da2df6d99672afb8a719642eca7
MSRV:                1.88
license:             Unlicense OR MIT
```

Primary source:

- <https://github.com/BurntSushi/ripgrep/blob/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/globset/Cargo.toml>

Accepted manifest:

```toml
globset = { version = "=0.4.20", default-features = false }
```

Exact transitive implementation includes `bstr`, `aho-corasick`, `regex-syntax` and `regex-automata`; the manager reconciles versions with exact-search donors in one lockfile lane.

Profile facts that must remain explicit:

- donor default `log` is disabled;
- no Serde/arbitrary/ignore filesystem walker;
- every `GlobBuilder` option is set, never platform default;
- product candidates use canonical `/`-separated relative bytes and `Candidate::from_bytes`, not lossy `Path` conversion;
- use a reusable result vector path, not per-observation convenience allocation;
- donor compilation is synchronous and not preemptively cancellable;
- donor internal NFA/cache behavior is implementation profile evidence, not ELIOT-configurable authority.

Glob match is classification only. It never proves containment, source identity, access or currentness.

# 4. Literal matching — `#246`

## Selected exact source

```text
memchr:             2.8.3
MSRV:               1.61
license:            Unlicense OR MIT
current lock cksum: cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98

aho-corasick:       1.1.5
MSRV:               1.60
license:            Unlicense OR MIT
```

Primary source:

- <https://github.com/BurntSushi/memchr/blob/2.8.3/Cargo.toml>
- <https://github.com/BurntSushi/aho-corasick/blob/1.1.5/Cargo.toml>

Accepted profile:

```toml
memchr = { version = "=2.8.3", default-features = false, features = ["std"] }
aho-corasick = { version = "=1.1.5", default-features = false, features = ["std", "perf-literal"] }
```

Use `memchr::memmem` for exact case-sensitive single literal. Use Aho-Corasick only where its explicit standard-match/overlap/ASCII-insensitive configuration exactly preserves the current contract. The chunk adapter remains ELIOT-owned and must prove:

```text
aggregate limits before scanning
overlapping matches
ASCII-only fold
matches spanning arbitrary chunk splits
one-extra-match truncation evidence
no whole-input concatenation
```

Reject regex as the single-literal baseline and reject convenience APIs that silently choose leftmost-first or omit overlaps.

# 5. Cargo graph/status tooling — `#250`

## Selected exact source with boundary correction

```text
cargo_metadata:     0.23.1
source tag:         0.23.1
MSRV at tag:        1.86
license:            MIT
default features:   none
```

Primary source:

- <https://github.com/oli-obk/cargo_metadata/blob/0.23.1/Cargo.toml>
- <https://github.com/oli-obk/cargo_metadata/blob/0.23.1/src/lib.rs>

Accepted manifest:

```toml
cargo_metadata = { version = "=0.23.1", default-features = false }
```

Do **not** use `MetadataCommand::exec()` as the bounded runner. Exact source invokes `Command::output()`, which captures complete stdout/stderr before selecting/parsing JSON. Accepted architecture:

```text
MetadataCommand::cargo_command()
→ add exact --locked --offline/--no-deps options
→ ELIOT bounded subprocess execution with one deadline and stdout/stderr byte caps
→ verify exit
→ parse only capped UTF-8 metadata through cargo_metadata::MetadataCommand::parse
→ closed ELIOT inventory/status checks
```

The crate depends on `serde_json` with its `unbounded_depth` feature available; `parse()` still uses normal `serde_json::from_str`. ELIOT caps bytes first and never treats parser failure as an empty workspace.

No Cargo graph, target, feature or package identity is inferred from raw TOML/lockfile scanning after cutover. Narrow literal workspace-pin/bridge-inheritance checks remain because metadata does not expose unused workspace pins or inheritance syntax; unused vendor patch/replace declarations are rejected explicitly.

### #250 archive and closure review (2026-10-09)

Archives were downloaded from crates.io, SHA-256 recomputed and compared with
the primary release records before the manifest/lock change:

| Donor | Archive checksum | Immutable source | License | MSRV |
|---|---|---|---|---|
| cargo_metadata 0.23.1 | `ef987d17b0a113becdd19d3d0022d04d7ef41f9efe4f3fb63ac44ba61df3ade9` | `c08e66cdf534313085ef810ce6f2e0df8a83fc50` | MIT (bundled LICENSE-MIT and immutable upstream) | 1.86 |
| camino 1.2.6 | `bbbad30e4b4c14a39e3cc8aed085a12a327257c316619c93581e017bc52be591` | `86ed28351d83fa7ef8287bcd5af36b8a4f073c0c` | MIT OR Apache-2.0 (bundled licenses) | 1.61 |
| cargo-platform 0.3.3 | `dd0061da739915fae12ea00e16397555ed4371a6bb285431aab930f61b0aa4ba` | `f2d3ce0bd7f24a49f8f72d9000448f8838c4e850` | MIT OR Apache-2.0 (bundled licenses) | 1.91 |

`cargo_metadata` defaults are empty; builder/unstable stay disabled. Normal
closure selects camino `serde1`, semver `serde`, serde derive, serde_json
`unbounded_depth` and thiserror. The reused lock already contains serde/core
1.0.229, serde_json 1.0.151, semver 1.0.28 and thiserror/impl 2.0.20 (including
syn 3 in the latter's proc-macro closure). Reused manifests' MSRVs are below
1.98. Camino's inspected build script probes `rustc --version` and emits cfg;
it performs no download. Cargo-platform has no build script. Development and
optional builder/proptest dependencies are not admitted as runtime mechanisms.

A bounded named-crate RustSec screen used recursive tree
`7eebec69c352c7191b1f13eb95dd510eeca5d1de` (2279 entries, untruncated) and found
no advisory paths for these selected donors or the inspected common closure.
This is not a full workspace security qualification. Post-lock full metadata
readback confirmed the three new versions/sources/checksums above, with empty
cargo_metadata/cargo-platform features and camino `serde1`. The 17-package
normal/build closure (excluding the inactive `cfg(any())` leg) reuses itoa
1.0.18, memchr 2.8.3, zmij 1.0.23, proc-macro2 1.0.107, quote 1.0.47,
syn 3.0.5 and unicode-ident 1.0.24 in addition to the named serde/error closure;
all reported MSRVs are below 1.98. The same RustSec snapshot contains no
advisory paths for those 17 names. This screen does not evaluate unrelated
workspace packages or provide a general security verdict.

The MIT license was checked at [the immutable source](https://github.com/oli-obk/cargo_metadata/blob/c08e66cdf534313085ef810ce6f2e0df8a83fc50/LICENSE-MIT).
Its Git blob is `31aa79387f27e730e33d871925e152e35e428031`; the bundled license
matches after CRLF normalization. The upstream Cargo.toml matches bundled
Cargo.toml.orig at blob `7f77850de76d361d1d416ec77bdcc7424677abde`.

# 6. Code identifiers — `#252`

## Selected exact source

```text
unicode-ident:           1.0.26
MSRV:                    1.71
license:                 (MIT OR Apache-2.0) AND Unicode-3.0
Unicode data:            18.0.0

unicode-normalization:   0.1.25
MSRV:                    1.36
license:                 (MIT OR Apache-2.0) AND Unicode-3.0
Unicode data generator:  18.0.0
```

Primary source:

- <https://github.com/dtolnay/unicode-ident/blob/1.0.26/Cargo.toml>
- <https://github.com/unicode-rs/unicode-normalization/blob/master/Cargo.toml>
- <https://github.com/unicode-rs/unicode-normalization/blob/master/scripts/unicode.py>
- <https://www.unicode.org/reports/tr31/>
- <https://www.unicode.org/reports/tr15/>

Accepted profile:

```toml
unicode-ident = "=1.0.26"
unicode-normalization = { version = "=0.1.25", default-features = false, features = ["std"] }
```

The exact release checksum and resolved data/table identity are recorded at lock time. The profile owns XID and NFC only. It does not implement Unicode full case folding, Turkic behavior, prose segmentation, confusable detection or language-specific identifier semantics. Old vectors remain a legacy profile; changed normalization/data requires a new projection generation.

# 7. Markdown/JATS — `#226`

## Pulldown-cmark correction

Exact `pulldown-cmark 0.13.1` tag has:

```text
MSRV:     1.71.1
license:  MIT
features: default = [getopts, html]
```

It does **not** expose the later `std` feature. Therefore the old issue pin with `features = ["std"]` is invalid.

Accepted exact pin:

```toml
pulldown-cmark = { version = "=0.13.1", default-features = false }
```

Primary source:

- <https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.1/pulldown-cmark/Cargo.toml>

Use `Parser::into_offset_iter()` for event/source ranges. Raw HTML/link/image destinations remain inert metadata under explicit policy; no HTML renderer/resource fetcher is enabled.

## Quick-XML

```text
quick-xml:  0.42.0
MSRV:       1.86
license:    MIT
default:    no features
```

Accepted exact pin:

```toml
quick-xml = { version = "=0.42.0", default-features = false }
```

Primary source:

- <https://github.com/tafia/quick-xml/blob/v0.42.0/Cargo.toml>

Explicitly absent:

```text
async-tokio
encoding
escape-html
overlapped-lists
serde-types
serialize
```

Use the pull reader over retained UTF-8 bytes with all structural checks enabled and bounded reusable buffers. `buffer_position()` and `error_position()` are reader positions, not automatic exact semantic node/text spans. The profile must derive ranges from before/after event positions under a frozen no-trim/no-transcode/no-entity-expansion policy and verify every claimed raw range against retained bytes. If exact mapping cannot be proved, cap assurance or reject the profile—never fabricate offsets.

## Sequencing

`#226` is `SELECTED_WITH_CORRECTION` and is blocked by `#257`. Markdown and JATS land separately. Neither parser owns UnitId, complete UnitSet, retained source, access, publication or worker isolation.

# 8. Unicode prose folding — outside Wave 2 coding

`#253` remains the decision owner and `#254` the later implementation. Do not smuggle full folding into `#252` or document profiles. The preferred small candidate remains an exact checked-in table generated from pinned Unicode 18.0.0 primary data, subject to the manager’s completed `#253` decision/goldens.

# 9. Donor acceptance checklist

Before each dependency commit, W2-SA02 returns and the manager verifies:

```text
exact package/version/tag/source commit
registry checksum
license and bundled-data terms
MSRV against Rust 1.98
normal/build/dev dependencies
explicit feature closure
advisories and maintenance state
API/source facts relied on
allocation/resource/cancellation limitations
public-type containment
network/runtime-download behavior
rejected alternatives
fixture/differential oracle
ACCEPT / REJECT / BLOCK
```

A checksum copied from an old lockfile is evidence only for that exact crate version. A branch README, Context7 summary or issue prose cannot replace exact source inspection.

## Final donor verdict

The Wave-2 mechanisms are sufficient and materially reduce custom code. The main corrections are sequencing and boundaries, not adding more frameworks:

```text
retain existing ELIOT authority/state machines
port small closed donor snapshots only
use exact small parser/matcher/Unicode crates privately
reject dynamic framework ownership
serialize all root pin/Cargo.lock changes through one manager
```

No additional large donor stack is justified for Wave 2.
