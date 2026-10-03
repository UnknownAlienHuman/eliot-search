# Progress after the Codex update

Product work resumed on 2026-10-03. The Goal is active. This record supplements the historical
[RESTART.md](RESTART.md) and [QUEUE.md](QUEUE.md); it is not a capability or qualification receipt.

## Delivered source

`main` was pushed and read back at `d963c7a6fe053c3b2b90a86f42777dfd0af52601`.

- The real bridge requires a purpose-bound, current API-key lease and a connection-binding provider
  before each RPC. Admission checks authenticated gRPC as well as server identity. A write whose
  lease expires before readback returns an unknown outcome, rather than a false rollback receipt.
- The bridge's four lib-test modules now use explicit paths to the intended test source.
- The reclaimer's disposable live fixture supplies the authenticated fixture binding and lease.
- Epoch-pin validation and removal retain their mutex protection; explicit drops follow the last
  protected operation.

Independent review accepted the authentication source and the subsequent harness/reclaimer/pin
delta. The latter review was performed on clean `d963c7a`; it did not claim runtime qualification.

## Checks performed

Windows x64, Cargo/rustc 1.98.0. On clean `d963c7a`:

```text
cargo +1.98.0 check --locked --offline -p search-index-reclaimer --all-features --test live_rebuild
cargo +1.98.0 clippy --locked --offline -p search-qdrant-bridge -p search-index-reclaimer -p search-epoch-pins --all-features -- -D warnings
cargo +1.98.0 check --locked --offline -p eliot-searchd --all-features
```

All exited 0. The reclaimer check ran 15:34:13–15:34:14 UTC, combined strict Clippy
15:34:14–15:34:15 UTC, and daemon check 15:53:38–15:53:41 UTC. The daemon still emitted
109 warnings; this is not daemon strict-Clippy acceptance.

The bridge directory on `d963c7a` is byte-identical to clean
`51390bf4114c7078cd133cec278d9ca2380ae58a`, where this single focused invocation ran:

```text
cargo +1.98.0 test --locked --offline -p search-qdrant-bridge --lib --test live_probe --test live_probe_module_ownership -- --nocapture
```

It ran 15:50:09–15:50:16 UTC and exited 0: 30 library tests, 3 live-probe tests, 2 ownership tests
and 13/13 mandatory disposable Qdrant probes passed. Wrong/missing credentials were rejected;
the valid leased key admitted the real plane; expiry after a write produced unknown outcome and
subsequent exact readback resolved the stored result. Pre/post observations found no surviving
owned Qdrant process, listener or temporary child directory.

The measured server was Qdrant 1.19.0, commit
`74f3e85b9473c62560006c043e13737ce6b48412`, executable length 84,184,576 bytes and SHA-256
`369c562eae3d89333a13abfdb522fa209e3f587c1217a1059d817e80814ea9d4`.
Client: pinned `qdrant-client` 1.19.0. The captured live-output SHA-256 is
`4f3447a2a0ccad222f4d0906f983ed4fea3f90c953a50eb27fe2f53c498d28f3`;
the fixture credential was absent from that output. Machine paths and raw captures remain outside Git.

## Current work and limits

The Windows supervisor is undergoing source safety fixes and strict-Clippy cleanup before independent
review and native execution. Package-owned `qdrant_data` configuration is being implemented with
enforced transport/batch bounds. Canonical S11 identity is prepared separately; its planner,
publication, bridge and daemon consumers must be ported coherently before that stack lands on main.

[Issue #199](https://github.com/UnknownAlienHuman/eliot-search/issues/199) still needs an explicit
policy/fence-generation encoding decision. [Issue #205](https://github.com/UnknownAlienHuman/eliot-search/issues/205)
still tracks exact transport for every normative epoch: the existing extreme-value probe does not
prove all intervening integers. No reduced epoch domain or provider upgrade was introduced.

The production daemon still lacks the real supervisor/bridge composition. Its test indexed port
does not enable production retrieval. W3 and installed-product qualification remain **UNQUALIFIED**;
native ownership, complete epoch/payload/eligibility qualification and the real
source → durable state → Qdrant → validated source-readback path are outstanding.

ELIOT/codebase-memory MCP tools were not exposed in this session. Source inspection and available
local structural tools were used; no unavailable semantic tool execution is claimed.
