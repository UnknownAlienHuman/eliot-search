# ELIOT Search — build the useful product, retire the scaffolding

**Source inspected:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`, 2026-10-18.
The [current architecture](../architecture/README.md) changes the local baseline. This is a refactoring
programme, not evidence that source changes have landed. [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352)
coordinates the existing manager; preserve #598/private WIP.

## 1. The immediate result

A user registers a repository, searches ordinary text through Qdrant, receives a useful file/range/snippet
and can reopen its exact revision. An edit updates the index; restart preserves the committed view;
index loss rebuilds projections; cancellation and source exclusion work through the same entrypoints.

The current default path serves DIRECT retained-source scans. Indexed composition is not connected to
the public query path. The source-v2 admission command records identity/admission and explicitly returns
`retained_revision:false`, `source_body_serving:false`, `ready:false`. Neither fact supports starting
another generic platform before completing the missing source-to-result path.

| Work through the real caller | Reuse what exists | Remove or stop requiring |
|---|---|---|
| Retain one admitted source and prepare it | #598 held-source work; current revision, materializer, unitizer and lexical code | Six-domain ownership transfer and generic key lifecycle as local ingestion prerequisites; parallel stores/selectors. Preserve already encrypted/sole-copy data. |
| Publish bounded Qdrant batches | S11/UnitSet/shared contracts/planner (#256–#259/#519); publication admission (PR #605); bridge vendor transport | Duplicate identity/payload/filter rules, mock authority and uncalled publication facades. #262/#261/#263 are actual contract/effect seams. |
| Serve an indexed result | `BlockingRealQueryPlane`, executor, source validator, result projector, typed client, delivered #213 evidence schemas | Another Tokio runtime, universal recipe registry, per-query exact count and test-report token needed merely to connect. |
| Connect one invocation/transport | Current CLI child-stdio path and reusable client; native OS pipe adapter where persistent serving is needed (#332) | Repeated argv/config capture and old line/TCP/session stack. Mandatory delegated pairing is not the trusted local baseline. |
| Prove useful behavior | Exact source inputs and the real application/Qdrant entrypoints | Feature flags, source-shape tests, receipts and closed issues presented as working search. |

These rows describe one path; they are not a new mandatory PR train. Integrate owners and immediate
consumers together where that finishes the result. Keep DIRECT explicit when indexed retrieval is
unavailable. Do not fabricate ready/grant/policy values to activate old abstractions.

## 2. Corrected source facts

- `search-contracts/src/evidence.rs` and provider `frame/client/evidence_schema.rs` implement the #213
  retrieval/orientation contracts; #213 is closed. Implement real serving, not another schema freeze.
- `search-qdrant-bridge/src/real/blocking.rs` already wraps one reusable Tokio runtime. The old daemon
  comment saying the async boundary is still missing is stale.
- Planner S11/UnitSet and canonical publication admission are delivered. Use the existing result and
  repair its real producer/consumer mismatch rather than repeating the algorithm port.
- Native source admission, stored ciphertext hashing and DEK selection are real bounded deliveries.
  They do not prove that retained source bodies, encryption and query serving are connected.
- Git loose-object validation exists; pack/ref/object acquisition remains a separate missing backend.
  Unicode XID/NFC, Markdown/XML, glob/matching libraries are already used. Donors replace only actual
  missing/duplicate mechanisms.

See the [verified audit](../audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) for paths and limitations.

## 3. Remove these mechanisms

| Area / existing issue | Refactoring decision |
|---|---|
| Legacy source-map migration (#268/#269 and related staging work) | Retire general staged import/cutover/rollback as the local default. Fresh format/root and rebuild; specific read-only export only for real retained user data. Never auto-delete the old root. |
| Daemon harnesses (#189) | Remove obsolete snapshot/sealed CLI/loopback applications and their exclusive callers/fixtures. Keep shared live code until its consumer moves. A test target is not a supported compatibility promise. |
| Root/command scaffolding (#266/#267/#332) | One stable OS root lock, transactional control row, one invocation/config and absolute deadline. Retire old-owner co-lock/succession/executable-continuity machinery with the old writer. |
| Generic secret/crypto programme (#307–#310/#330) | Encryption and restricted-client credentials are selected features. One actual key-to-writer/readback path when needed; no default gate on unused rotation/catalog/receipt machinery. Preserve #598 work and all existing encrypted data. |
| Access/query composition (#274/#288/#290/#291/#293/#294/#279) | Trusted local scope and pinned view; call the existing retrieval/readback path. Remove uninstantiated authority/handler trees. Keep actual scope filtering/restriction. Exact counts/proofs belong to explicit exhaustive operations. |
| Publication/reclaim | Keep one pending publication record, real effect/readback and committed route. Reclaim from actual live manifests/pins; optional backups/legal holds/ownership transfer do not gate ordinary indexing. |
| Historical tooling (#214/#525) | Remove committed coverage/package maps, issue-state/source-hash ledgers and their gate coupling. Do not replace the 72k-line digest ledger with a slightly smaller ledger. Retire the partial Rust import/module taint analyser too; keep behavioral hash/identity, actual Cargo dependency checks and adapter API review. |
| Copied algorithms (#315/#316/#329/#262) | Reuse shared hashes/identity/schema and delete old producers. For retired transient plans, invalidate/recompute; for incompatible projections, rebuild. No automatic migration framework for every changed hash. |
| Packaging/features | Drop delivery-wave names and optional-feature chains from the default app. Consolidate modules after deletion; keep independent OS/vendor/worker dependencies where useful. |

Counts in the audit locate costs; they are not a promise that every line in an area can be deleted.
A successful reduction removes state/transitions/callers, not merely test files or code formatting.

## 4. Preserve optional scope explicitly

The analysis mission remains. Add depth through the same source/preparation/query/result path after it
works. An old issue can remain open for a selected feature without blocking the local baseline.

| Capability | Useful next outcome |
|---|---|
| Orientation and exact verification | Serve delivered evidence/orientation types; recommend concrete reads/checks. Freeze/scan a denominator only for an explicit exhaustive claim. |
| Code inspection/comparison (#223/#224/#225/#232/#296–#298) | Import Tree-sitter/SCIP facts or bounded heuristic symbols, resolve ambiguity and compare independent lineage. Avoid another graph/index authority. |
| Git (#129/#335) | Narrow local object/pack/ref acquisition over admitted storage using gitoxide mechanisms; no hooks/fetch/checkout runtime. |
| Documents (#216/#226/#227) | Reuse current Markdown/JATS. Add selected HTML/PDF/Office/OCR profiles with exact coordinates and bounded isolated workers; no universal parser prerequisite. |
| External corpora (#228–#231) | Bounded versioned observations/checkpoints with exact source identity and explicit gaps. |
| Restricted clients/adapters (#194/#219) | Real delegated scope enforcement for a client that needs less authority than the OS owner; ELIOT/Research/MCP remain leaf consumers. |
| Revision encryption and durable exports | A selected confidentiality/retention need, one consumed implementation, explicit recovery of existing data; not mandatory local bootstrap. |
| Dense/rerank/overlay | Optional measured profiles. No LLM/model/runtime dependency in the lexical/code baseline. |

Paired backup, full historical migration, legal holds and cross-provider ownership transfer are optional
capabilities. The old completion condition requiring all of them for any useful release is superseded.

## 5. Verification and release

[Implementation guidance](EXECUTION_PROTOCOL.md) selects checks from the actual change. For a connected
application, measure source-to-hit correctness, source reads/context needed by the agent, incremental
updates, restart/index loss, cancellation, resource use and scope exclusion. #215/#264 remain places for
real Windows/Qdrant/scale evidence; their reports are not runtime credentials.

A release describes the capabilities it actually ships and their measured limits. Omitted profiles are
explicitly unavailable. No audit-wide issue closure, ledger, waveform feature flag or review badge can
substitute for a working installed application. Publication remains a separate explicit action.
