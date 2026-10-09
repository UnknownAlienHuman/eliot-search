# Agent launch gate — 2026-10-09

**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97).  
**Audited implementation snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Current execution packet:** [Wave 2](WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md).  
**Scope exceptions:** [package matrix](WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md).

## Current verdict

```text
Wave 1 #237 / PR #319:                 MERGED
Wave 1 #253 / PR #322:                 MERGED
Wave 2 A1 #258 / PR #323:              MERGED
Wave 2 A2 #250 / PR #326:              MERGED
Next controlled implementation:        #256
Entire Wave 2:                         NOT COMPLETE
Installed product / release:           NOT QUALIFIED
```

Earlier text saying that #237 had not merged or Wave 2 had no base is historical. Do not rerun completed tasks or reset an existing manager worktree to an earlier launch SHA.

## Start or resume

The manager reads the latest accepted-base comment on #97 and the current packet. At this audit the code snapshot is `17383079d316cc090eae04912534e2d16e6dc79d`; subsequent documentation-only commits do not constitute new source-check evidence. If implementation has advanced, inspect the delta and use the latest accepted code base instead of overwriting work.

One manager owns the integration worktree, commit/merge decisions and root pins/Cargo.lock. Use 5–10 bounded subagents for the active slice. Delegated write permission must follow the actual maintainer instruction and reconciled root/package scopes; a report's assertion of permission is not a replacement for that instruction. No second integration manager or concurrent lockfile writer.

## Remaining order

```text
#256 → #257 → #266 → #235
→ #238 → #241 → #246 → #252
→ #226 Markdown → #226 JATS
```

The first two Wave-2 stages (#258 and #250) are already accepted. #327 is a separate small security dependency repair through the same manager before affected TLS/network use; do not mix it into point-identity code. #325 remains explicit legacy bounds-provenance debt.

## Build-safe source gates

#256 adds the new S11 profile without immediately deleting legacy exports still imported by the daemon. The current eight-field key, full identity, independent address domain and collision rejection use the already merged canonical API. Existing legacy callers are retired by the named downstream cutover, not by breaking compilation in a package-only PR.

Run the active issue's locked Rust 1.98 check and strict Clippy with applicable all-target/all-feature flags. Compile affected reverse consumers when a public API changes. Record pre-existing failures separately from new regressions.

After source/ledger changes:

```text
cargo +1.98.0 run --locked -p xtask -- validate canonical-digest-guard
cargo +1.98.0 run --locked -p xtask -- validate qdrant-boundary
```

#324 repairs the known stale evaluation README; the full current-workspace command must be rerun after that repair. No success is inferred merely from changing the text. Actions stay manual-only; broad product/native tests remain deferred.

## Evidence already recorded

- #319 final head `99918613044952be7a0b1a919aa99bf5ba0e8cfb`: author records Windows Rust 1.98 scoped check/Clippy and source guard success.
- #322: exact Unicode-data decision and golden artifacts; no production prose tokenizer.
- #323: closed indexed schema/eligibility and epoch fixtures; no live Qdrant enablement.
- #326 final head `5c796c91a32d67a4113f83c14ad625c0ec314165`: author records scoped check/Clippy, 176 focused fixtures and guard success. The merged snapshot has the same file tree.
- #326 also records a real workspace-validator exit 1 for #324, not a blanket PASS.

This audit inspected source and recorded evidence; it did not independently rerun Rust or Windows checks. Security-review badges and Git signatures are not compiler or functional evidence.

## Do not manufacture progress

Do not widen a legacy profile into S11, replace an old checksum with a domain-prefixed one under the same fingerprint, disable a failed checker, change ledger classifications to obtain PASS, or reopen completed foundations as a new architecture project. The packet and exact issue define the smallest next code change.
