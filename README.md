# ELIOT Search

Local-first source preparation and retrieval for ELIOT, implemented as a **standalone Rust search
product around Qdrant**.

ELIOT Search can run independently through `eliot-searchd` and `eliot-search`. Integration with ELIOT
Memory OS is optional and uses typed provider contracts over the same standalone product owners.

## Product boundary

ELIOT Search owns:

- source admission, revision retention and preparation;
- lexical/projection construction;
- Qdrant supervision, schema, publication and retrieval;
- access-scoped query planning, candidate validation and result projection;
- currentness, handles, rebuild, retention and recovery;
- standalone daemon/CLI and optional provider adapters.

It does **not** own Tasks, WorkScopes, GM/Governor state, agent scheduling, native harness lifecycle,
assignment tickets, writer leases, mailboxes, review acceptance or finish authority. Those responsibilities
belong to [`eliot-swarm-controller`](https://github.com/UnknownAlienHuman/eliot-swarm-controller) and,
after integration, [`eliot-memory-os`](https://github.com/UnknownAlienHuman/eliot-memory-os).

The repository's `swarm/**` files are historical/advisory development metadata. They are not runtime
inputs and are not prerequisites for implementing or running Search. See ADR 0005.

## Architecture baseline

- Rust 1.98 workspace;
- Qdrant is the only indexed/search database;
- redb stores bounded technical control state, not a searchable corpus;
- immutable source/preparation evidence remains outside Qdrant;
- Qdrant payload is projection metadata and never authoritative source evidence;
- standalone DIRECT remains independent of indexed-mode availability;
- ELIOT integration is a leaf adapter, not a reverse authority path.

The normative implementation master is:

```text
docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md
```

## Current status

Implementation is incomplete and this is not a qualified production release.

The active completion work is the real product spine:

```text
source admission / immutable revisions / preparation
→ typed projections
→ real Qdrant publication
→ access-scoped retrieval and filtered IDF
→ authoritative candidate readback and validation
→ bounded standalone/provider results
```

Indexed readiness remains disabled until the exact Qdrant artifact, bridge, schema, query, restart and
end-to-end qualification gates pass. Planning records, source presence and mock/oracle execution are not
qualification.

## Workspace

The workspace contains product libraries under `crates/` and supported binaries under `bins/`.
Key entrypoints:

- `bins/eliot-searchd` — long-lived standalone daemon/composition root;
- `bins/eliot-search` — local client/CLI;
- `crates/search-index-qdrant/search-qdrant-bridge` — sole Qdrant vendor adapter;
- `crates/search-index-qdrant/search-publication` — epoch-safe publication coordinator;
- `crates/search-query/*` — access, planning, retrieval, validation and result projection;
- `crates/search-source/*` — admission, identity, revisions, materialization and unitization;
- `crates/search-control-redb` — bounded technical control state.

## Build

Use the pinned Rust toolchain and locked dependencies:

```powershell
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 clippy --workspace --all-targets --all-features --locked -- -D warnings
```

During implementation, run the smallest applicable package gate first:

```powershell
cargo +1.98.0 check --locked -p <package> --all-features
cargo +1.98.0 clippy --locked -p <package> --all-features -- -D warnings
```

Full product/process tests are performed after the implementation spine is complete, with focused tests
added earlier where a change needs immediate causal proof.

## Running

The supported baseline is standalone. Exact startup syntax is still evolving with the provider transport,
redb cutover and Qdrant supervisor integration; do not infer readiness from legacy experimental commands.

The final baseline must support:

- installation and first startup without ELIOT Memory OS;
- source registration/ingestion;
- durable restart-safe state;
- real Qdrant indexing and search;
- rebuild after index loss;
- local CLI use;
- optional ELIOT provider integration without a second state owner.

## Repository workflow

Read `AGENTS.md`, the architecture master, the nearest package instructions and the named issue/PR before
editing. Use one branch/worktree per manager and avoid overlapping package writers.

No ticket/lease/signature issuance protocol is required. Do not implement generic agent-controller
features in this repository. Keep GitHub Actions manual-only and never claim unexecuted gates.

## License

See [`LICENSE`](LICENSE).
