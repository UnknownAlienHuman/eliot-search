# PR #142 — reviewed findings and first repairs

Audit input: `47ae26aa7cc3127e54daf951ecd43486dc78c57f`, reviewing
`b80483b82554747ab0b15afe7c3415ffcde00c75`.

**The central criticism is valid:** implemented libraries are not the implemented
product when the daemon does not call them. More standalone publication/control
machinery must not substitute for a compiling, exercised primary execution path.

## Repairs in this increment

- **D-1:** provide the missing raw/hex/decode/multipart SHA-256 API in the existing
  module. Keep the original raw implementation and newtype; no new dependency.
  The concrete `&[&[u8]]` argument supplies the required coercion context for the
  three mixed-array/vector calls. The first multipart framing is explicitly
  specified in `docs/runtime/DIRECT_HASH_FORMAT.md`, not asserted as recovered history.
- **D-2.1:** the redb successor fixture uses `i64::MAX - 1`, the largest valid
  Epoch; the prohibited sentinel remains prohibited in production.
- **D-2.2:** the redaction fixture uses nonce/ciphertext patterns distinct from
  public digests and checks ordinary and pretty Debug, with positive dump controls.
  No ciphertext leak was demonstrated; the production formatter is unchanged.
- **Additional fixture defect:** the publication integration test had the same
  forbidden `epoch(i64::MAX)` twice. It now uses the largest valid value. Claude's
  `--lib` run did not exercise this test target; this is source-confirmed, not a
  newly executed failure report.
- **Actual dead source:** remove `bins/eliot-searchd/src/service_state.rs`, an
  uncompiled alternative lifecycle journal. Repository searches for the module
  and `ServiceJournal` found no consumer; its sole executable reference was
  `include_str!` in a source guard. Remove that one text check, retaining the eight
  live/harness identity sites and all behavioral/native/process tests.

## Qualifications to the audit

D-3's identifier graph is useful evidence of missing integration, not compiler-
resolved reachability and not proof that all 38 unconnected libraries are disposable.
Preserve required capability owners and their regression oracles; remove a local
implementation only after its replacement covers the actual public path.
`cargo check --workspace --lib` checks workspace library members even when they are
not default daemon dependencies. `--all-features` remains required for feature-
specific paths, but is not the only way to type-check those library members.

D-4 correctly requires an S31 boundary decision. Eleven libraries is the recommended
layout, not an unconditional count cap. Do not merge/split packages merely to alter
counts or evade the 10,000-line hard stop. The redb fixture repair adds no Rust lines.

D-5's statement that no PR was ever merged is incorrect: GitHub records #98 as merged
on 2026-09-05, with head/merge SHA `fbacea30e3d5bd168ab9d111b013adde369dce9a`.
Zero merge commits does not prove zero merged PRs. It still does not establish
independent review or an accepted handoff. Owner-directed main integration likewise
cannot manufacture launch/gate acceptance or erase the stale registry reconciliation.

## Required next gate, before extending disconnected code

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --workspace --lib --locked --no-fail-fast
cargo +1.98.0 test --locked -p search-publication --all-targets
cargo +1.98.0 test --locked -p eliot-searchd --all-targets
cargo +1.98.0 test --locked -p search-control-redb --doc
```

Run the primary CLI/daemon smoke on a fresh disposable root, then native Windows
fixtures. Library-only success is insufficient. Once executable, select one real
DIRECT source/control/preparation integration slice, replace its local duplicate,
and repeat its process tests. Full formatting/Clippy, package-boundary review,
registry reconciliation and real Qdrant integration remain open.

Claude reports executed baseline commands in #142. This repair was source-reviewed;
its Rust checks remain **NOT_RUN** because Cargo is unavailable (actual attempt:
exit 127). The independent multipart digest calculation and uploaded-blob/diff
checks do not replace compilation or qualify any runtime behavior. No task or
architecture gate is accepted by this document.
