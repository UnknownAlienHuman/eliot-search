# Saved checkpoint for the Codex update

Saved at the maintainer's request on 2026-10-03. Stop after delivering this checkpoint; resume product
implementation after the update and a continuation request. The product Goal is paused, not completed.
Read [QUEUE.md](QUEUE.md) for the reviewed PR/task order.

## Source retained

| Scope | Exact source and saved branch | Status at this checkpoint |
|---|---|---|
| Accepted main source | `bf22e4c302766b6aa78e5e26d7d37242807dac30`; also `codex/qdrant-daemon-build-gate` | Daemon module paths, exports and authority lifetimes repaired in place. All-features check passed. Strict Clippy failed in dependencies; daemon lint cleanliness and runtime remain unverified. The commit containing this file adds documentation after this checked source. |
| Source identity/registry | `f473dfdafe03a93e2d91f8afb99cf9e41ed14985` and `a43e8ed0202a8c103d0011b8d1d0e9ba3a919045`, included in main | Small digest parsing/documentation lint repairs. Exact package checks and strict Clippy passed. |
| Authenticated bridge | `a8e9f527b5f02be985edf150aa3748b50291524b`, `codex/qdrant-auth` | Independent source review accepted; check and strict Clippy passed. Not integrated into main. The only focused test invocation failed while compiling the lib-test harness; no tests or live Qdrant proof ran. |
| Native Windows supervisor | `d2aed304dc97fe2e05e8484bc7d9d46ee05623c8`, `codex/qdrant-supervisor-native-contained` | Windows all-features check passed without warnings. Strict Clippy failed with 78 diagnostics. Source review and native execution remain acceptance blockers. Not integrated into main; runtime NOT_RUN. |

The three product branches above were pushed and read back at their exact SHAs before this document
was committed. They preserve source, not qualification. Do not merge unaccepted native code merely
because it compiles. Preserve all other worktrees and refs, including unrelated dirty work.

## Checks actually executed

Windows x64; Cargo/rustc 1.98.0. Commands are package-scoped. The external local checkpoint records
machine paths, exact times, raw captures and hashes; those files are intentionally outside Git.

Daemon, at `bf22e4c`:

```text
cargo +1.98.0 check --locked --offline -p eliot-searchd --all-features
cargo +1.98.0 clippy --locked --offline -p eliot-searchd --all-features -- -D warnings
```

Check exited 0 at 13:08:17 UTC, with 109 warnings. Clippy exited 101 at 13:08:42 UTC on dependency
lints, including search-control-redb, search-provider-protocol, revision-store, access, handles,
epoch-pins, OS-secrets and unitizer. No daemon lint locations appeared before dependency compilation
stopped; this does not establish a clean daemon Clippy run.

Bridge, at `a8e9f527`:

```text
cargo +1.98.0 check --locked --offline -p search-qdrant-bridge --all-features
cargo +1.98.0 clippy --locked --offline -p search-qdrant-bridge --all-features -- -D warnings
cargo +1.98.0 test --locked --offline -p search-qdrant-bridge --lib --test live_probe --test live_probe_module_ownership -- --nocapture
```

Check and Clippy exited 0 at 13:30:03 and 13:30:46 UTC. The single test invocation ran
13:48:02–13:48:12 UTC and exited 101: 65 test-harness compile errors, beginning with missing
`context`/`errors` modules in `src/real/tests.rs`, then unresolved vendor/test symbols. There were no
test-result lines, spawned Qdrant processes, observed listeners or temporary child directories.
This is a compilation failure before execution, not a failed live-auth assertion. No retry occurred.

Supervisor, at `d2aed304`:

```text
cargo check --locked --all-features -p search-qdrant-supervisor --target x86_64-pc-windows-msvc
cargo clippy --message-format short --locked --all-features -p search-qdrant-supervisor --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

The clean final Windows check exited 0. The captured Clippy inventory exited 1 with 78 diagnostics.
These commands used the observed Cargo/rustc 1.98.0; their execution timestamps were not captured.
The source-bound external captures preserve that limitation. No supervisor process was launched.

## First work on continuation

1. Fetch, inspect main and each saved branch, and preserve dirty worktrees. Read the current root and
   owning package instructions. Architecture Part I and accepted ADR 0005/0006 govern standalone
   Search; generic controller tickets, writer leases, profiles and PKI are not prerequisites.
2. Repair the bridge's existing lib-test module paths/import visibility on its saved branch. Preserve
   the auth tests for correct/wrong/missing credentials, expiry/binding mismatch, redacted diagnostics
   and post-write unknown-outcome recovery. Compile the repaired harness before the single necessary
   focused auth run. Do not repeat unchanged failed commands or start a broad product test loop.
3. Adapt the reclaimer's `tests/live_rebuild.rs` old connection call when integrating bridge auth.
   The new API requires endpoint, current binding, API-key lease, qualified gate and limits. Disposable
   fixtures provide `fixture_connection_binding()` and `fixture_api_key_lease()`; these are explicitly
   synthetic, not proof of native process ownership.
4. Close the supervisor's native source-review findings and owned Clippy diagnostics before runtime
   acceptance. Keep the real Job/ACL/identity checks, pre-execution Job assignment, no-breakaway policy,
   callback-only secret lease and guarded unknown outcomes. Do not replace them with caller booleans
   or silence diagnostics by weakening the safety boundary. Review the corrected exact SHA.
5. Resolve the #199/#205 contract decisions and port the ordered #207 → #209 → #210 stack; reconcile
   the competing bridge #200/#202 changes. Then implement owner configuration and daemon composition
   in the order in QUEUE.md. Do not launch new agent/framework features before this product spine.

## Runtime and qualification boundary

Qdrant server 1.19.0 is installed. Its executable is 84,184,576 bytes with SHA-256
`369c562eae3d89333a13abfdb522fa209e3f587c1217a1059d817e80814ea9d4`.
The client remains pinned to qdrant-client 1.19.0. Installation and earlier disposable diagnostics
do not establish the exact complete server/client/profile qualification: W3 remains **UNQUALIFIED**.

The production daemon still lacks the real supervisor/bridge composition and package-owned
qdrant_process/qdrant_data configuration. Its test indexed port is not a production substitute.
Next composition must obtain the real OS-secrets purpose-bound lease and retained current process
guard; public binding fields and synthetic fixture leases are not OS evidence. The SDK copies an API
key into its interceptor; no complete SDK memory-zeroization claim was established.

Keep source identity, root containment, currentness/access, exact point identity/payload, publication,
readback and unknown-outcome recovery in their actual owners. Product completion still requires the
source → durable state → real Qdrant → exact validated source path, public recipe/handle behavior and
native/restart/deny/rebuild qualification.

## Reviewed documentation proposal

[PR #222](https://github.com/UnknownAlienHuman/eliot-search/pull/222) is still OPEN at
`a669248737ac8982db70eb54b78ab17b8ddb199b` (verified 2026-10-03). It is a 19-file documentation
proposal based on `50b1e1b`, not merged product behavior. Its status snapshot and evidence references
need refreshing. The #220 status/evidence and normative extraction work is not complete. PR #97's
historical dispatch/audit snapshot does not describe current main. Do not restore its controller gates.
