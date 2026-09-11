# Next-agent task: product tests for the finished product

Base: `main` at creation time. Read first: `AGENTS.md`,
`qualification/CURRENT_FAILURES.txt`,
`docs/runtime/QDRANT_NATIVE_WINDOWS.md`,
`qualification/qdrant/W3_QUALIFICATION.md` (13-step order).

## 1. Close the known test gaps (no mocks-as-live)

- Indexed-search CLI verb: T30 records that no daemon command performs an
  indexed Qdrant search; add the verb plus e2e (gated, unavailable when
  unqualified — never empty success).
- gRPC parity: daemon tests speak loopback REST; prove the qualified
  `qdrant-client` 1.19.0 gRPC path with the same assertions
  (`search-qdrant-bridge --test real_dataplane` is the reference).
- Access-gate provider wiring: `access_composition` is allow-listed as
  proven-but-unwired (see `entry.rs` comment); wire it once a
  grant-issuing authority exists — inventing authority is forbidden,
  so this item starts with designing grant issuance, not with code.
- T41 tail: port the remaining Python validator families slice by slice
  (pattern: xtask port + parity vectors + delete-only-replaced);
  `workflow_dispatch`-only always.
- Qdrant re-qualification: execute the W3 13-step order against a fresh
  pinned install and collect reviewer receipts (this flips
  `artifact.toml` to QUALIFIED — only with receipts, never by editing).

## 2. Evidence lanes that are still UNAVAILABLE

- Linux lane: full `check/test/clippy` on Linux (native DPAPI tests stay
  Windows-only and must be marked so, not faked).
- 10k read-only queries sustained run; frozen-corpus perf re-measurement
  after any refactor (T40 ceilings are measured, keep them measured).
- Release (T43): exact artifacts, checksums, install/start/search/restart/
  recovery guide executed verbatim, independent exact-byte review.

## 3. Standing test hygiene (every run)

- `cargo test --workspace --all-targets --all-features --locked
  --no-fail-fast` green; honest clippy count (`level==error` +
  `is_primary`, never `grep -c`); `cmdkey` revision-key 0/0;
  no `qdrant` orphans (`Get-Process qdrant`); temp roots cleaned.
- Refresh `qualification/CURRENT_FAILURES.txt` with measured numbers only;
  unavailable stays UNAVAILABLE, never zero.
- Push to `main` promptly in small commits; never lose work.
