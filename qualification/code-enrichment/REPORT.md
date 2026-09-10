# T35 — code enrichment qualification report (unit/golden level)

**Base:** `2a728b6` (`2a728b675a837e84e5993c3a4e27a38495422a0c`), branch `main`.
**Head:** uncommitted working tree (no commit, no push per task rules).
**Toolchain:** `cargo/rustc 1.98.0`, `x86_64-pc-windows-msvc`.
**Predecessors:** T16 manifests and T31 currentness accepted in `main`.

## Write scope (respected)

- `crates/search-prep/search-code-enricher/src/lib.rs` (modified)
- `crates/search-prep/search-code-enricher/tests/t35_enrichment_qualification.rs` (new)
- `qualification/code-enrichment/ARTIFACT_PIN.toml`, `REPORT.md` (new)
- `Cargo.toml` / `Cargo.lock`: **untouched** (parser pin is document-only).
- Parallel agent T32 paths (`bins/eliot-searchd/**`) were not read for
  implementation and not modified.

## Design

The approved external parser artifact does not exist yet, so this change
qualifies the built-in no-execute tolerant baseline at unit/golden level and
records exactly what is missing for full qualification (see below). No stub,
placeholder, silent fallback, second symbol database, or compiler-truth claim
was added.

1. **`validate_fact_anchor` (new public API, required by `FUNCTIONS.md`).**
   Reopens the exact retained `RustRepresentation` for one `StructuralFact`
   and checks source-revision/representation identity plus byte-range and
   UTF-8 character-boundary mapping. Foreign identity fails with
   `AnchorMappingFailed`; out-of-range or split-character ranges fail with
   `StructuralFactUnmapped`. Returns a byte-free `AnchorValidationReceipt`
   (identities + digests + range + profile digest), so every emitted anchor
   reads back exact retained coordinates and provenance.
2. **`const`/`static` items are facts again.** The qualifier stripper removed
   `const`/`static` before classification, making those declaration arms
   unreachable (`pub const MAX_POINTS` produced no fact). `const` is now kept
   unless it qualifies `fn` (`const fn`/`const unsafe fn`/`const async fn`
   still classify as functions); `static` is never stripped.
3. **Nameless declarations degrade explicitly.** A declaration with no
   recognizable (ASCII) identifier now emits `MissingIdentifier` and a
   recovered node instead of a silent complete parse (e.g. `fn привіт() {}`).
   Anchors stay byte-exact; assurance stays `DescriptiveOnly`.
4. **Projection mapping, not a symbol DB.** Facts carry only existing shared
   vocabulary (`EntityKind::ALL`, `EvidenceRole::ALL`,
   `AssuranceClass::DescriptiveOnly`); resolved relation targets must already
   exist inside the same representation, otherwise the target stays an
   explicit unresolved name with `ambiguous = true`.

## Tests (TDD: first run failed to compile on the missing API — E0432)

`crates/search-prep/search-code-enricher/tests/t35_enrichment_qualification.rs`,
**26 tests, all passing.** Coverage:

- golden corpus pinned: 22 facts / 24 relations, full
  (name, kind, role, entity) multiset, determinism (`enrich_code` twice
  byte-equal), manifest count consistency;
- Unicode (Cyrillic doc + `connecter` byte-slice readback; non-ASCII name
  recovery), CRLF line mapping, replacement character;
- macros observed-not-expanded (`macro_rules!` definition vs invocations;
  `include_str!`/`emit!` reference-only; fact count proves no expansion);
  generated-marker file stays bounded and descriptive;
- malformed input degrades (`UnbalancedDelimiter`, `RecoveryNode`,
  `MalformedSource` gap, `DegradedTolerantSyntax` assurance) and `Reject`
  policy returns `ParseDegraded`;
- immediate and mid-stream cancellation return `ParseCancelled` with no
  manifest; node/fact/step budgets fail closed with `ParseBudgetExhausted`;
- `cfg`/`cfg_attr` `all`/`any`/`not`/key/key-value stay distinct and
  unevaluated; truncated `cfg` → explicit `Unknown`, unbalanced nesting →
  `ConfigurationAmbiguous`; guarded facts link predicates without claiming
  inclusion;
- profile validation (`ParserProfileInvalid` for floating versions,
  `no_execute = false`, empty editions, bad limits; `ParserNotQualified` for
  digest/checksum/golden mismatches), edition/size rejection, non-UTF8
  rejection;
- anchor readback for every golden fact + foreign-identity and unmapped-range
  rejection;
- profile-change classification (`Noop` / `ReEnrichAndReproject` /
  `GateRequired` / `Reject`);
- projection-vocabulary gate and ordinal retained-unit binding.

## Executed evidence

```text
cargo +1.98.0 test --locked -p search-code-enricher --all-targets
# exit 0 — 26 passed, 0 failed

cargo +1.98.0 clippy --locked -p search-code-enricher --all-targets -- -D warnings
# exit 0 — zero warnings

cargo +1.98.0 fmt -p search-code-enricher -- --check
# exit 0 — own files clean (scoped package fmt only; bare `cargo fmt` never used)
```

`cargo check --workspace` is left to the controller (out of write scope).

Own diff (`git diff --stat` filtered to scope):

```text
crates/search-prep/search-code-enricher/src/lib.rs | 73 ++++-
crates/search-prep/search-code-enricher/tests/t35_enrichment_qualification.rs | NEW (~1020 lines)
qualification/code-enrichment/ARTIFACT_PIN.toml | NEW
qualification/code-enrichment/REPORT.md | NEW
```

`forbid(unsafe_code)` preserved; no `todo!()`, placeholder, or unbounded
queue introduced.

## What is missing for full qualification (not claimed)

1. **Approved external parser artifact + Cargo.lock pin** — grammar/vendor
   selection, exact version, source checksum, and license receipt require an
   integration-owner ticket with executed evidence. This package must not add
   the dependency. `ARTIFACT_PIN.toml` records `lockfile_change = "NONE"`.
2. **Executed no-execute probe** — static absence of process/network calls is
   by construction; a runtime-monitored probe (no Cargo/rustc/build-script/
   network/credential activity during real parser runs) needs integration
   fixtures and stays `UNAVAILABLE` in `qualification/rust-syntax/probes.toml`.
3. **Real parser runs** — no approved artifact is available locally, so per
   task rules only unit/golden level is claimed; no `UNAVAILABLE` stub gate
   was added.
4. **T16 unit-span cross-check** — facts bind retained units ordinally
   (`unit_ids[min(index, len-1)]`); verifying units against exact unitizer
   span manifests needs the accepted T16 handoff bound by the controller.
5. **Baseline extraction limits (documented, not fixed here)** — no field
   facts, no `use`/path/call relations, `Contains` dormant under
   line-granular ranges, single-line scanner (multi-line signatures classify
   on their first line), ASCII-only identifier recognition, UTF-16/loss-map
   interplay owned by the materializer.
6. **Frozen production profile/golden digests** — test vectors stand in for
   measured artifact digests; freezing them and wiring the profile-change
   gate into projection regeneration is controller/integration work.
7. **Workspace check, review receipt, and downstream readback** — controller
   runs `check --workspace`; independent review verifies the head; post-merge
   readback precedes any downstream release.
