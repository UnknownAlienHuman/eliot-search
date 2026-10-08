# Saved continuation after the 2026-10-03 closeout

The maintainer requested that the current work be saved and the session stopped. The Goal is paused.
Start the next session here, then use [QUEUE.md](QUEUE.md) for the remaining product obligations.
[PROGRESS.md](PROGRESS.md) and [RESTART.md](RESTART.md) retain earlier observations; their old active-work
wording and candidate revisions do not override this checkpoint. This record is not product qualification.

## Delivered main

Source revision `3a5d92fd9338e4ecbdda3a615e081534f1b8fcbf` was pushed to `main` and verified by remote
readback. It includes the previously delivered bridge-authentication, daemon topology, epoch-pin,
reclaimer and control-redb repairs, plus three independently reviewed owner slices:

| Package | Accepted owner revision | Result and remaining boundary |
|---|---|---|
| search-materializer | `a54466cdc9183d993130cd7e84fc23ca9227a45a` | Actual BLAKE3, framed v2 identities, direct exact-byte source/output hashes and retained-byte verification. `ELIOT-MAT-V2`; old immutable artifacts require rebuild from retained source. Modern typed preparation producers remain incomplete. |
| search-unitizer | `a7d5196517970063dc114a65f32dfa0f0a7b30bc` | Actual BLAKE3 and `ELSUMF02`; reject non-BLAKE3 tags and overflowing/oversized record counts before allocation. Legacy v1 is explicitly unsupported pending rebuild. No complete typed UnitSet producer was added. |
| search-source-registry | `51a6f875d6b77693a5233cca943fb5ca7b5a303e` | Typed immutable revision occurrences, replay-safe operation lookup, expected-head comparison and exact predecessor-ID validation. Production durable source-admission/ID providers remain missing. |

At this exact integrated source revision, Windows x64 with Rust/Cargo 1.98.0:

```text
cargo +1.98.0 check --locked --offline -p eliot-searchd --all-features
cargo +1.98.0 clippy --locked --offline -p search-materializer -p search-unitizer -p search-source-registry --all-features -- -D warnings
```

Both exited 0. The daemon check ran 18:53:29.527–18:53:33.162 UTC and still emitted 109 warnings;
its stderr SHA-256 is `929b9395a6436c18449d311be4935de8d6c4059dae3ee75ff5ac7cbb1199f19d`.
The owner Clippy gate ran 18:54:01.466–18:54:03.071 UTC; its stderr SHA-256 is
`b2db6c7fe067714e02a1dd517a8629b285f8ce97e9c70dfcade622fb95542311`.
Raw captures and exact metadata remain outside Git under the closeout evidence label for `3a5d92f`.

Focused owner proofs already ran on their clean revisions: materializer 12 profile, 9 product,
12 UTF-8 and 1 baseline integration cases; unitizer's earlier v2 slice 33 package cases followed by
3 decoder-closedness cases after the audited repair; source-registry 4 revision cases after the
predecessor repair. Package check and strict Clippy passed on each final owner revision. These are
bounded package proofs, not persisted migration, recovery or end-to-end qualification. Source-revision
gate commands/outcomes were reported by the owner; standalone raw captures and per-gate times were
not retained. The closeout's integrated Clippy capture independently verifies that package's strict gate.

## Saved branches outside main

Keep these worktrees and local refs. Do not merge the entire divergent stacks into main or reset dirty
external worktrees. The machine-specific continuation record locates every worktree and raw capture.

| Branch | Saved revision | State |
|---|---|---|
| `codex/qdrant-data-config-resume` | `3a79500cb9585c1e1a3b9b1198cf0137c4b1981a` | Clean package checkpoint; bounded transport runtime qualification pending. |
| `codex/qdrant-supervisor-native-contained` | `8a0ff5668c39f30a40b066be2288e8c1f500bb75` | Clean native source checkpoint; Windows check and strict Clippy passed. Final independent source review and native runtime qualification remain pending. |
| `codex/s11-daemon-composition` | `69ba90da250a728931e7f6b823402cad7f0485a6` | Clean integration checkpoint; check passes, strict gate and binary test compilation fail. |
| `codex/point-identity-s11-resume` | `adb2127e938ff32f499b12e51e00e611c259ec91` | Clean S11 identity owner slice; check, strict Clippy and 9 focused cases passed. |
| `codex/projection-s11-resume` | `db88cada02db54b91bbe7c82c9d8c25018bde121` | Clean planner owner slice; check, strict Clippy and 8 focused cases passed. Complete representation-bound unit-set proof remains absent. |
| `codex/publication-s11-resume` | `dfd6afd66afc84e78cc0dd0a48bf99e9c4e4e8d5` | Clean publication owner slice; check, strict Clippy and two individual safety proofs passed. Daemon/Qdrant composition remains unverified. |

The new bridge checkpoint adds package-owned `qdrant_data` defaults, validation, digest, change planning
and apply receipt. Its private Tonic transport bounds messages to 8 MiB, enforces operation deadlines
and batch limits, and requires an already connected, authorized socket provider on every connection.
The exact tree is `fdbf07cf3080e391273ff2ff36f57b821121c37b`: check, strict Clippy, lib/live-probe
compilation without execution, four config cases, one oversized decoder case and scoped rustfmt passed.
No Qdrant process ran for this new adapter. The older SDK's 13-probe receipt cannot qualify it.

Bridge composition must register `section_descriptor`, validate settings and pass them through
`RealDataPlane::connect_with_settings`; the native owner must supply the production
`QdrantConnectedSocketProvider`. `apply_live_settings` acknowledges only that plane's update and does
not prove global daemon reload. The selected disposable bounded-transport proof is still pending.

The native checkpoint uses fresh monotonic lease ticks, exact rendered-config BLAKE3, retained physical
directory observations and an owner-binding callback. Authorization checks the established TCP tuple
against the child PID and creation marker before exposing the key. Each launch consumes a non-cloneable
`VerifiedExecutable` token produced by the bounded contained version preflight. A concrete daemon root
provider and `qdrant_process` section are still missing; these source mechanisms do not qualify runtime.

On exact native `8a0ff56`, these Windows-target gates both exited 0:

```text
cargo +1.98.0 check --locked -p search-qdrant-supervisor --all-features --all-targets --target x86_64-pc-windows-msvc
cargo +1.98.0 clippy --locked -p search-qdrant-supervisor --all-features --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

Check was captured at 19:00:20 UTC, log SHA-256
`15497ba422e7cb1432d64b2bc0d02434892f715acd497299124a6f2d95d24290`.
Strict Clippy ran 19:00:28–19:00:29 UTC, log SHA-256
`27bbe7ffb801d8efb713867e3f350bd39871d11a2ceb322af6e551571589956e`.
Tests were compiled by the all-targets gates but not executed. Qdrant runtime remains on hold.

## S11 integration failures to resume from

At exact `69ba90da250a728931e7f6b823402cad7f0485a6`:

- Daemon check with all features passed at 18:34:49–18:35:15 UTC, with 109 warnings.
- Strict daemon Clippy exited 101 at 18:39:51–18:39:53 UTC: two `unused_self` errors in
  `search-os-secrets-windows/src/credential/windows.rs`, lines 118 and 171. No strict daemon acceptance.
- A mistaken projection filter on `--lib` ran zero tests. Its exit 0 is not a projection proof.
- The corrected binary invocation exited 101 at 18:46:16–18:46:21 UTC before executing tests:
  53 compilation errors, including missing split-module test paths and unresolved access-composition
  test imports. The new CAS crash-stage cases have not executed.

Resume the failed commands only after the relevant source repair:

```text
cargo +1.98.0 clippy --locked --offline -p eliot-searchd --all-features -- -D warnings
cargo +1.98.0 test --locked --offline -p eliot-searchd --all-features --bin eliot-searchd projection_composition::
```

The root CAS adapter writes and syncs a unique sibling stage before atomic no-clobber publication and
exact bounded readback. Source review accepted this repair; production protected-root/ACL ownership
and runtime crash/recovery remain unqualified. The branch contains the earlier unitizer `9b01161`
slice; bring in the accepted final `a7d5196` repair and current main before further integration.

## First steps next session

1. Fetch; verify main, saved refs and all worktree statuses. Read Architecture Part I, ADR 0005/0006,
   current root/package instructions and the named issues. Keep Search standalone. Historical ticket,
   lease or controller records cannot block product implementation.
2. Review the final native checkpoint and enforce its preflight/clock/config/root/socket boundaries.
   Supply the actual retained root guard and full 128-bit child-directory identity in daemon composition.
   Release the child mutex before async RPC/reconnect; do not hold a Rust lock across network work.
3. Finish package-owned `qdrant_process` configuration, register both Qdrant sections, and compose the
   qualified child, current purpose-bound secret lease and bounded bridge. Perform the selected real
   native/bounded proof after source review; do not reuse an old SDK receipt for the changed transport.
4. Repair the saved S11 integration gates and bind a complete ordered typed UnitSet to the exact
   `Representation.unit_manifest_digest`. Freely supplied `expected_units` is not that authority.
5. Add durable typed Active source admission plus qualified source-ID/revision-ID minting, atomic
   redb append/head/operation readback and modern preparation producers. Legacy caller maps, booleans,
   ordinals or digest truncation cannot stand in for those records.
6. Resolve [#199](https://github.com/UnknownAlienHuman/eliot-search/issues/199) eligibility/payload
   encoding and [#205](https://github.com/UnknownAlienHuman/eliot-search/issues/205) exact full-i64 epoch
   transport before indexed qualification. The external composite integer-filter proposal is unaccepted.
   Then demonstrate real source → durable state → Qdrant → validated source readback, including deny,
   restart, unknown outcome and rebuild. Continue the remaining ordered issues in [QUEUE.md](QUEUE.md).

The later queue remains #218 corpus/portfolio → #213 free-text/orientation → #221 ranking → #219 agent
adapter and actual Luna repository study → #215 scale / #216 qualified documents → installed baseline.
The documentation proposal #222 and controller removal #214 remain separate work; neither is runtime
acceptance or a controller prerequisite.

## Qualification boundary and shutdown

Qdrant 1.19.0 is installed. The previous SDK adapter has bounded historical authenticated probe evidence
recorded in [PROGRESS.md](PROGRESS.md); it does not establish the new transport, native lifecycle, full
W3, installed baseline or production daemon pipeline. Those remain **UNQUALIFIED**.

No new source-admission feature was started at closeout. No new Qdrant runtime was admitted. Raw logs,
credentials and local machine paths stay outside committed documentation. Agents finish their current
checkpoint and stop; the Goal stays paused until the maintainer resumes.
