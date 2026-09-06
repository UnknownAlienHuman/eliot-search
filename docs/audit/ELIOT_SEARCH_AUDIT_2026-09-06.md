# ELIOT Search: current-head build, test and integration audit

Date: September 6, 2026. Audited commit: `b80483b82554747ab0b15afe7c3415ffcde00c75`.
Toolchain: `rustc 1.98.0 (88d9e12ae 2026-08-18)`, Linux x86_64.

This is a measurement report. It accepts nothing, releases no task, advances no launch state and
creates no receipt. Every count below is reproducible from the commands in
[Verification boundary](#verification-boundary). Machine inventory:
[`2026-09-06-inventory.json`](2026-09-06-inventory.json).

It continues [`ELIOT_SEARCH_AUDIT_2026-09-04.md`](ELIOT_SEARCH_AUDIT_2026-09-04.md), which named the
central gap as "the distance between these modules and an actually runnable daemon". That gap is now
measured rather than described.

## Verdict

Two blocking defects and one structural finding.

1. `eliot-searchd` does not compile. It has never compiled in this repository's history.
2. Two library tests fail on `main`, both in packages modified on 2026-09-06.
3. Only 5 of 43 library crates are reachable by code from any binary. 38 crates and 58,200 lines of
   Rust have no path to a product target.

Library compilation itself is clean: `cargo check --workspace --lib --locked` returns 0 errors for
all 43 libraries. The defects are at the integration boundary, not inside the packages.

## D-1. `eliot-searchd` does not compile (blocking)

`cargo check --workspace --all-targets --locked` fails:

| Target | Errors |
|---|---|
| `eliot-searchd` bin | 87 |
| `eliot-searchd` bin test | 94 |
| `eliot-search-sealed-authority` test | 13 |

Error classes: 84 × `E0425` (name not found), 3 × `E0308` (type mismatch).

Single root cause. `bins/eliot-searchd/src/sha256.rs` (blob `ed4bd239`) exposes `digest_bytes` and a
`Sha256Digest` newtype. Fifteen daemon modules call four functions that the module does not define —
`sha256::hex`, `sha256::digest`, `sha256::digest_parts`, `sha256::decode_digest`:

| Module | Errors in the `eliot-searchd` bin target |
|---|---|
| `direct_store.rs` | 27 |
| `secure_direct_store.rs` | 8 |
| `direct_store_ingest.rs` | 8 |
| `app.rs` | 7 |
| `result_handles.rs` | 6 |
| `continuation.rs` | 6 |
| `maintenance.rs` | 5 |
| `directory_manifest.rs` | 5 |
| `source_fence.rs` | 4 |
| `secure_direct_store_storage_io.rs` | 4 |
| `revision_protection.rs` | 3 |
| `storage_security.rs`, `service_output.rs`, `secure_commands.rs`, `public_runtime_service.rs` | 1 each |

`sha256.rs` has been written exactly once, in the repository import commit `772267d`. That same tree
already contained 90 call sites for the four absent functions; at `b80483b` there are 105. The
mismatch is not a regression introduced by recent work — the primary daemon target has never built
here.

Consequences that must not be reported as unverified-but-probably-fine:

- every build and test command in `README.md` and every step of `QUICKSTART.md` is unrunnable;
- `.github/workflows/manual-workspace-check.yml` fails at its first step, so its later steps
  (`core_tests`, native identity, process regressions, the eight retained legacy harnesses) have
  never reported on current head;
- no statement in `README.md` about daemon behaviour — persistent root registration, catalog-loss
  rejection, proxy retirement, fail-stop on uncertain mutation — is currently verifiable.

The three `E0308` sites are a separate defect that a module-API repair alone will not close.
`continuation.rs:303` and `result_handles.rs:278` build `&[&self.session_nonce, &counter.to_be_bytes()]`,
whose elements are `[u8; 32]` and `[u8; 8]` and cannot unify; `direct_store.rs:745` passes `&[u8; 8]`
where `&Vec<u8>` is expected. The intended argument shape of the absent `digest_parts` is not
recoverable from the current tree and must be decided, not guessed.

## D-2. Two library tests fail on `main` (blocking)

`cargo test --workspace --lib --locked --no-fail-fast`:

### D-2.1 `search-control-redb` — impossible fixture

`persistent::publication::visibility::tests::succession::constructor_rejects_skips_reused_identity_stale_guards_and_overflow`
panics at `visibility/tests/succession.rs:127`:

```
called `Result::unwrap()` on an `Err` value:
ContractError { code: EpochOutOfRange, kind: EpochOutOfRange, field: "epoch" }
```

The line is `exhausted.target_epoch = Epoch::new(i64::MAX).unwrap()`. `Epoch::new`
(`crates/search-contracts/src/ids.rs:212`) validates the half-open range `0..i64::MAX`, so `i64::MAX`
is rejected unconditionally. The test can never pass as written.

Introduced by `8670732` (2026-09-06). Result: 218 passed, 1 failed, 5 ignored.

The intent — proving that a successor cannot be reserved once the epoch domain is exhausted — is
sound and should be retained. Only the fixture value is wrong: the exhausted epoch is
`i64::MAX - 1`, the largest constructible `Epoch`.

### D-2.2 `search-revision-store` — false-positive redaction assertion

`tests::encrypted_payload_debug_never_dumps_ciphertext` fails at `src/lib.rs:841` on
`assert!(!debug.contains("[7, 7"))`.

**There is no ciphertext leak.** The `Debug` implementation at `src/lib.rs:238` redacts both
sensitive fields as `<N bytes>` and `<N encrypted bytes>`. The assertion trips on the *digest*:
`Blake3Digest32` is produced by the `digest_newtype!` macro (`search-contracts/src/ids.rs:309`) with
a derived `Debug` over `[u8; 32]`. The fixture `payload(7)` fills the plaintext digest, the nonce and
the ciphertext all with the byte `7`, so `Blake3Digest32([7, 7, 7, …])` is printed — correctly, it is
not secret — and matches the substring before any ciphertext could.

The assertion therefore proves nothing about redaction. Result: 8 passed, 1 failed.

The fix is to make the fixture discriminate — a byte pattern for the ciphertext that appears in no
non-secret field — so that the assertion tests the property it claims. Weakening or deleting it would
remove a security regression guard.

## D-3. The workspace is not one product (structural)

### D-3.1 Only five library crates are reachable from a binary

Reachability computed over actual Rust identifier references (`src`, `tests`, `benches`, `examples`
of all 47 workspace members plus the workspace-level `tests/`), transitively from the four binaries:

| | Crates | `.rs` lines |
|---|---|---|
| Libraries reachable from a binary | **5** | 12,796 |
| Libraries not reachable from any binary | **38** | 58,200 |
| Libraries with no external consumer at all | **26** | 43,518 |

The five reachable libraries are `search-contracts`, `search-exact`, `search-lexical`,
`search-materializer`, `search-unitizer`.

The largest packages with no consumer anywhere in the tree:

| Crate | `.rs` lines |
|---|---|
| `search-control-redb` | 9,991 |
| `search-config` | 4,354 |
| `search-eval` | 4,232 |
| `search-runtime-owner` | 3,401 |
| `search-publication` | 2,911 |
| `search-comparator` | 1,855 |
| `search-code-enricher` | 1,740 |
| `search-overlay` | 1,688 |

The single occurrence of the identifier `search_control_redb` in the entire tree is inside its own
doc-comment example (`snapshot_guard.rs:22`).

Cargo dependency edges exist but are not exercised. `search-config` is declared by 22 manifests and
appears 10 times in Rust code. `search-provider-protocol` is declared by 7 manifests and appears 0
times. Declared edges are not integration, and `cargo metadata` cannot distinguish the two.

### D-3.2 The daemon reimplements what it already links

`bins/eliot-searchd` declares seven non-optional workspace dependencies it never references:
`search-domain`, `search-ports`, `search-config`, `search-runtime-owner`, `search-os-secrets`,
`search-control-redb`, `search-provider-protocol`. They are linked and never called.

The daemon's complete set of workspace imports is eight `use` statements across 25,393 `src` lines:

```text
search_contracts    2   search_lexical  3   search_materializer 1
search_unitizer     1   search_exact    1
```

Everything else it needs, it has written again. Grouped by implementation line:

| Line inside `bins/eliot-searchd/src` | `.rs` lines |
|---|---|
| `sealed_*` and `src/bin/` — sealed prototype | 6,606 |
| local re-implementations (`control_store.rs`, `source_roots.rs`, `continuation.rs`, `result_handles.rs`, `revision_protection.rs`, `maintenance.rs`, `directory_manifest.rs`, …) | 5,400 |
| service / runtime | 5,013 |
| `direct_*`, `secure_direct_*` — DIRECT | 4,303 |
| `snapshot*`, `main.rs`, `lexical/`, `development.rs` — snapshot/BM25 experiment | 4,071 |

Row two duplicates the responsibilities of `search-control-redb`, `search-source-registry`,
`search-continuation`, `search-handles`, `search-revision-store`, `search-retention` and
`search-projection-planner`. Row five is the experiment `README.md` already declines to treat as a
product index.

`README.md` states that `PersistentControlJournal` performs real redb I/O but the primary source
catalog has not switched to it. The measured position is stronger: the daemon does not reference the
crate at all, so the switch is not a migration of a shared owner but an unstarted integration.

### D-3.3 Most of the workspace is not compiled by default

`bins/eliot-searchd/Cargo.toml` sets `default = ["wave1-shell"]`, and `wave1-shell = []` is empty.
The 26 crates behind `wave2-source` … `wave7-lifecycle` are all off. A default
`cargo build -p eliot-searchd` links the 12 directly declared non-optional libraries and calls 5.

Combined with the leaf adapters and workers outside the binary graph, 32,280 `src` lines are absent
from a default build. `--all-features` is therefore not an optional thoroughness flag here; it is the
only configuration in which most of the repository is type-checked at all.

### D-3.4 Provisional implementations counted twice

Part of the wired code is deliberately a model that a later task must replace or wrap:

- `search-qdrant-bridge` (803 lines) is by its own module doc "the in-memory model here … for a
  concrete adapter". The real transport is T24 / PR #121.
- `search-control-redb` carries `reference.rs` (1,011 lines, in-memory reference model) alongside
  `persistent/` (8,307 lines, real redb). Keeping a reference oracle is a defensible choice under
  S36; it does mean one responsibility is implemented twice.

## D-4. Package granularity diverges from S31

`docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md` S31 recommends eleven library crates
plus four binaries:

```text
search-contracts  search-domain  search-control-redb  search-source  search-prep
search-lexical    search-index-qdrant  search-query   search-runtime
search-eliot-adapter  search-eval
```

and states the admission rule directly: *"A capability cell becomes a separate crate only when it has
a real dependency, replacement, test or context boundary."*

`swarm/crates.toml` registers 45 packages; `Cargo.toml` declares 47 members. Nearly every capability
cell C00–C30 became its own crate, and the S31 crate names became directories: `crates/search-query/`
holds 11 crates, `crates/search-index-qdrant/` 7, `crates/search-source/` 6, `crates/search-runtime/`
4 plus 2 unregistered, `crates/search-prep/` 3.

No record in `swarm/` applies the S31 admission test to the resulting cells. The split may still be
right, but it is currently asserted rather than justified against the normative rule.

Its cost is measurable. Each package carries a manifest, an error enum, boundary newtypes and
conversions, `README.md` / `FUNCTIONS.md` / `AGENTS.md`, a module packet, a stage read-set, coverage
rows and a qualification packet:

| Layer | Lines |
|---|---|
| Rust (`crates` + `bins`) | 100,792 |
| TOML registries (`docs`, `swarm`, `qualification`) | 176,108 |
| Markdown (`docs`, `swarm`, `qualification`) | 24,438 |
| Markdown inside packages (165 files) | 12,930 |

The control plane is twice the size of the code it governs.

## D-5. Divergences already recorded elsewhere, still open

- **Launch state contradicts the tree.** `swarm/launch-state.toml` reports `active_stage = "P00"`,
  `active_wave = 0`, one authorized package, 42 blocked, and `issued_tickets`, `active_leases`,
  `submissions`, `accepted_reviews`, `accepted_package_handoffs` all `0`. `swarm/stages.toml` marks
  W1–W10 `BLOCKED`. `swarm/implementation-program.toml` is `PLANNED_NOT_AUTHORIZED`. Meanwhile
  100,792 lines of Rust exist, including in blocked packages. `swarm/tickets/`, `leases/`,
  `submissions/`, `reviews/`, `handoffs/`, `wave-receipts/` and `context-manifests/` contain only
  `README.md`. The two authorities cannot both be current.
- **Manifest and registry disagree.** 47 members against 45 registered packages;
  `search-revision-crypto` and `search-os-secrets-windows` have no package, function, module or stage
  entry. Open T01 blocker, recorded in `swarm/reconciliation/2026-09-05.json`.
- **Line budgets exceeded.** `AGENTS.md` sets a 10,000-line hard stop. `eliot-searchd` holds 25,393
  `src` lines against a 6,500 target; `search-control-redb` 9,991 against 7,500, past the 8,500
  split-review threshold.
- **No pull request has ever been merged.** All 50 commits reached `main` directly; the history
  contains 0 merge commits. The submission / review / handoff mechanism in `AGENTS.md` has never
  produced a record.

## D-6. What is correct and should not be re-litigated

- `README.md` and `QUICKSTART.md` are honest about status: incomplete implementation, in-memory
  Qdrant bridge, unfinished redb migration, preparation recomputed at query time, and an explicit
  "no fresh passing Rust build or test run is claimed".
- All 27 workflows are `workflow_dispatch` only. The Actions policy in `AGENTS.md` holds.
- The T02 target isolation is applied: `autobins = false`, one `[[bin]]`, eight legacy prototypes
  retained as `[[test]]` targets rather than deleted.
- `search-control-redb` depends on `redb = "=2.6.3"`; the 2026-09-04 finding about a redb-named
  package without a redb dependency is closed.
- All 43 libraries type-check.

## Ordering

D-1 and D-2 come first because they are small, local and block every other measurement. Nothing about
runtime behaviour, Windows security, Qdrant or performance can be established while the primary
target does not build and `main` carries failing tests.

1. **D-2.1, D-2.2.** Two fixture repairs in packages currently under active work. Keep both test
   intents; correct the values.
2. **D-1.** Reconcile the `sha256` module API with its 105 call sites, including the three `E0308`
   sites. Until this closes, `--all-targets` cannot go green and no daemon claim is checkable.
3. **D-3.** Decide, per capability, whether the daemon module or the crate is the owner, then delete
   the loser. Every task from T24 onward assumes crates that the daemon does not currently call;
   landing more of them without wiring increases the unreachable set.
4. **D-5.** Reconcile `swarm/launch-state.toml` and `swarm/stages.toml` with the tree, or record
   explicitly that the swarm ticket protocol is suspended in favour of the T01–T43 queue. Two
   contradicting authorities make the `AGENTS.md` read and write rules unenforceable.
5. **D-4.** Apply the S31 admission test to the 45-package split and record the outcome, before the
   remaining waves multiply the registries further.

## Verification boundary

Executed on `b80483b` with `rustc 1.98.0`, Linux x86_64:

```sh
cargo check --workspace --lib --locked                      # 0 errors
cargo check --workspace --all-targets --locked              # FAIL, see D-1
cargo test  --workspace --lib --locked --no-fail-fast       # 2 suites FAIL, see D-2
cargo fmt --all -- --check                                  # 1,715 diffs
```

Reachability, line counts and the consumer graph were computed over `.rs` files of all 47 workspace
members and the workspace-level `tests/`, by Rust identifier reference. Blob identities of the
inspected sources are recorded in [`2026-09-06-inventory.json`](2026-09-06-inventory.json) so a later
run cannot silently use another baseline.

Not performed, and not claimed in any form: `--all-features` compilation, Windows native execution,
DPAPI, any Qdrant runtime, `cargo metadata` dependency-kind resolution, clippy, benchmark or
performance measurement, and independent review. A failure counted here is a current-head fact; an
unavailable check is recorded as unavailable, never as zero.
