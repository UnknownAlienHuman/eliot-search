# Analysis registry

Audit continuation memory: inspected source, findings, changes, execution evidence and unresolved questions.
Not a permission gate, product specification or completion percentage. A statically established defect and an
implemented correction are different states.

**Original public baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `ff59f930c5b160b1ed36684538be3dca1bdd6970` (`R022`, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`

Resume from the actual PR head and inspect intervening commits. Bind execution claims to exact SHA, command and
target. Condensation must preserve immutable links to detailed prior passes. Default-branch search discovers
paths; it does not establish current-branch caller absence.

## Status vocabulary

- **CHECKED**: the named static question was inspected, not every line/runtime property.
- **PARTIAL**: findings exist; named inspection or implementation work remains.
- **PENDING**: insufficient inspection to direct a safe change.
- **RUNTIME-ONLY**: code/execution are necessary; prose is not evidence.
- **PROTECTED**: unpublished work is not inspected or overwritten.

## Coverage ledger

Detailed earlier surfaces and findings remain in the immutable checkpoints below. All area IDs are retained.

| ID | Area | Status | Established result / current state | Remaining |
|---|---|---|---|---|
| A001 | Documentation authority | PARTIAL | Historical parallel specs removed. R022 removes client instructions making cancellation optional/synchronous and reconciles process ownership and terminal semantics. | Other surviving local documents/issues and useful old requirements against code. |
| A002 | Historical governance data | CHECKED | Swarm/maps/packets/generated status removed; Git is archive. | Code remnants below. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions manual-only. | Recheck changed workflows, not a new gate. |
| A004 | Workspace inventory | CHECKED for inventory | R020 removed retention member/dependency/lock shell; 50 declared members. | Actual Cargo resolution and remaining package cuts. |
| A005 | Entrypoints/product | PARTIAL | R021 corrected daemon help/self-test claims. R022 traces CLI entry/public interception/app/core and finds another process/diagnostic path bypassing session bounds. | One actual default daemon/CLI and binary compilation; no claim of indexed integration. |
| A006 | Root/config/control | PARTIAL | One root guard and typed state selected; captured durable build inputs differ from live snapshots. | Current state/capture and platform durability. |
| A007 | Sources/reconciliation | PARTIAL | R010 defects recorded; R021 confirms live source-root currentness consumer in provider. | Source/object/current-row and published-versus-desired freshness; actual #598 overlap only. |
| A008 | Private object/key work | PROTECTED | Public contract reviewed; old six-domain/purge programme not inherited. | Accepted handoff; no private branch/worktree changes. |
| A009 | Preparation/identity | PARTIAL | Test/limit identity coupling, budget gap, empty refusal and stale/ABA distinctions established. | Correct actual preparation callers. |
| A010 | Qdrant/donors | CHECKED for named static questions | Gate removed; BM25/tokenizer/IDF/readback/late-write limitations recorded. | Real connection and relevance/scale/late-write execution. |
| A011 | Publication/view/cleanup | PARTIAL | Two-sided reconciliation, immutable retrieval and durable captured inputs selected. | Crash/late-writer/view/rebuild/cleanup code. |
| A012 | Query/evidence/window | PARTIAL | Evidence/window failures and unconstructed generic service inputs recorded. | Direct Qdrant/evidence/window integration and remaining callers. |
| A013 | Protocol/client/CLI | PARTIAL | R022 establishes replay-capacity cancellation refusal, expired-data cancellation refusal, synchronous-write starvation risk, late stdout-only capture check and conflicting client instructions. Documentation corrected; Rust remains. | Actual serviced session/caller cutover; native/remote/legacy core internals and platform execution. |
| A014 | Product depth | PARTIAL | Exact/navigation/comparison/documents/models/MCP remain requirements. | Concrete producers/consumers and quality assessment. |
| A015 | Tests/governance code | PARTIAL | xtask/ledger and inspected structural/disconnected proofs removed. R022 adds/removes no tests. | Mixed migration behavior extraction and residual structural consumers. |
| A016 | Donors | PARTIAL | Prior storage/search seams retained. R022 reads current Rust process/Instant docs and Microsoft pipe-mode semantics; no upgrade or pinned runtime claim. | Concrete remaining donor questions only. |
| A017 | Qualification tree | CHECKED | Archived; current policy/blocker notes remain. | Runtime ceremony A022. |
| A018 | Auxiliary documents | CHECKED for filename/disposition | Duplicate supplements removed; retention path is notes only. | Semantic consistency A001. |
| A019 | Issues/PR handoff | PARTIAL | Named issues are reconciled per pass, not a whole-queue check. R022 affects #280/#332. | Remaining owner issues as inspected. |
| A020 | Runtime evidence | RUNTIME-ONLY | Static findings and deleted fixtures do not establish platform/recovery/performance. | Exact-SHA relevant runs after implementation. |
| A021 | Compiler baseline | PARTIAL | Last previously verified bridge-lib run: 39872058276 at 7d92fb8b. No R022 compiler/test/runtime run; current container has no cargo/rustc. | Locked metadata and actual daemon/CLI/current consumers; Windows separately. |
| A022 | Runtime qualification/identity | PARTIAL | Qdrant activation removed; lexical/materializer fixture authority remains. | Real caller cleanup; causal examples stay tests. |
| A023 | GC/storage diagnostics | PARTIAL | Generic platform/Cargo shell removed; current collector and query inventory removal unfinished. | Typed stopped-root collection and no hot-path deep inventory. |
| A024 | Backup/restore | PARTIAL | Fresh destination/archive design recorded; old generic restore gone. | Actual archive/import and partial/corrupt/interrupted/protected cases. |
| A025 | Features/harness topology | PARTIAL | Wave7/full-baseline removed. R022 reads CLI entry feature gates but does not complete the remaining feature/consumer graph. | Wave1–wave6 and native/remote/shared consumers before further Cargo cuts. |

## Remaining static audit

1. Finish current-branch wave1–wave6 and shared sealed/migration/query callers. For CLI, next inspect the remaining
   `native_*`, `remote_client`, `endpoint_client`, `provider_client/{core,local}` implementations and their actual
   daemon counterparts; R022 inspected roots/selected loops, not their complete caller closure.
2. Reconcile surviving README/AGENTS/FUNCTIONS and affected issues against code; preserve useful old requirements
   without adding another global specification. R022's named client/protocol/CLI cluster is reconciled, not all docs.
3. Finish lexical/materializer fixture-activation and configuration-authority callers.
4. Separate causal filesystem cases from source-text assertions in mixed migration tests.
5. Resolve concrete exact/navigation/comparison/document/model/MCP producer/donor seams.

## Known implementation work, not unread audit scope

Connect one command application and retained source/object/current-state/preparation path; apply recorded source,
budget, empty/gap, stale-completion and lost-wakeup fixes. Connect Qdrant/publication, real pending effects,
immutable retrieval, atomic evidence/view, rebuild and cleanup. Connect one query/evidence/window/framed session
with actual cancellation and child ownership. Implement stopped-root GC/archive/fresh import. Remove replaced
mechanisms with their consumers and run minimal pinned compiler/Clippy. Runtime claims wait for execution.

R022 corrects seven Markdown files only. No production code, Cargo graph, transport, tests or actual capture/
cancellation behavior is changed. Do not mark an old Rust defect fixed because its replacement instructions changed.

## Immutable detailed evidence

These checkpoints retain exact inspected paths and execution limits. Current recipes, not superseded historical
instructions, govern implementation.

| Pass | Detailed checkpoint |
|---|---|
| R001–R012 | [44d7416c](https://github.com/UnknownAlienHuman/eliot-search/blob/44d7416c46554456f16ccd2b3943657b755304ee/docs/audit/ANALYSIS_REGISTRY.md) |
| R013 | [2b37f5ef](https://github.com/UnknownAlienHuman/eliot-search/blob/2b37f5ef1e8cc755bbcd8c0e1f7efc840eff8cf3/docs/audit/ANALYSIS_REGISTRY.md) |
| R014 | [8cf331a4](https://github.com/UnknownAlienHuman/eliot-search/blob/8cf331a44decded2c84c7f579dcbf8434e040dc7/docs/audit/ANALYSIS_REGISTRY.md) |
| R015 | [bf2bde29](https://github.com/UnknownAlienHuman/eliot-search/blob/bf2bde299b3f009527397571bd9baeea51fde840/docs/audit/ANALYSIS_REGISTRY.md) |
| R016 | [53ab0ea3](https://github.com/UnknownAlienHuman/eliot-search/blob/53ab0ea33f8d405f72e984881b5aea0db69e7c3b/docs/audit/ANALYSIS_REGISTRY.md) |
| R017 | [15145667](https://github.com/UnknownAlienHuman/eliot-search/blob/151456672ea4ec8773ed65c5017cb63989912d30/docs/audit/ANALYSIS_REGISTRY.md) |
| R018 | [174f3ec7](https://github.com/UnknownAlienHuman/eliot-search/blob/174f3ec758cb0ec5c38b780cd3de913be9120fdc/docs/audit/ANALYSIS_REGISTRY.md) |
| R019 | [156528ad](https://github.com/UnknownAlienHuman/eliot-search/blob/156528ad0a2ab64599d9cef01d3eeea2b6f11040/docs/audit/ANALYSIS_REGISTRY.md) |
| R020 | [e6c7bb90](https://github.com/UnknownAlienHuman/eliot-search/blob/e6c7bb902c9aa91509ceeccae996c610f2b8f0cd/docs/audit/ANALYSIS_REGISTRY.md) |
| R021 / full previous ledger | [ff59f930](https://github.com/UnknownAlienHuman/eliot-search/blob/ff59f930c5b160b1ed36684538be3dca1bdd6970/docs/audit/ANALYSIS_REGISTRY.md) |

## R022 — client control liveness and subprocess bounds (2026-10-23)

### Exact inspected scope

Source head `ff59f930c5b160b1ed36684538be3dca1bdd6970`:

- CLI `AGENTS.md`, `FUNCTIONS.md`, `src/{entry,app,provider_client}.rs`;
- CLI `src/public_client.rs` lines 1–150: interception, spawn and beginning of interactive loop;
- CLI `src/app/core.rs`: help, daemon-path/options, bounded-line reader, `invoke_stdio`, `invoke_daemon`, forward,
  dispatch including self-test and `run_main`; the remainder of endpoint parsing/tests is not a full-file claim;
- client `AGENTS.md`, `README.md`, `FUNCTIONS.md`, `src/typed.rs`, `src/typed/{io,state}.rs`;
- protocol `AGENTS.md`, `README.md`, `FUNCTIONS.md`, `src/{session,config}.rs`;
- Windows transport `src/lib.rs` including platform selection, flush and result classification; not `windows.rs`;
- continuation `FUNCTIONS.md`, PR #610 and issues #280/#332.

Directory listing and default-branch Cancelled-body search were discovery only. No whole-repository caller
absence or complete feature closure is inferred from them.

### Findings, corrections and limits

| Finding | Source / counterexample | Disposition |
|---|---|---|
| Replacement instructions recreate blocking behavior | Client AGENTS required a small synchronous client with optional cancellation; the protocol/CLI required active control during work. | Client AGENTS/README/FUNCTIONS now require one serviced session; only the outward API may be synchronous. Progress remains optional. |
| History capacity blocks control | SessionMachine inserts IDs without removal and applies max_replay_entries to admit_cancellation too; default is 4096. The final admitted operation can exhaust history before its Cancel. | Do not repair this by a larger ledger. Local replacement uses non-reused session IDs, one active request and bounded control, without completed-ID history. Old Rust unchanged. |
| Expired work prevents cancellation transmission | State::send_cancel takes min(new deadline, pending_deadline); after pending expiry remaining fails, and with_state drops the connection. | Distinguish control during active work from bounded cleanup after expiry. An aligned stream may send best-effort control during cleanup; otherwise close. Never claim sent cancellation or rollback. |
| Read polling alone is insufficient | SocketIo::write_parts loops through all parts and flush until completion/deadline; the same mutable state cannot poll receive/control during that call. | Selected runtime services both directions and keeps bounded partial-write state. No byte-level insertion of Cancel into an unfinished frame; finish within bound or close. This is a liveness counterexample, not a reproduced deadlock. |
| Diagnostic capture bypasses output bounds | CLI core self-test calls Command::output, then checks stdout only; stderr is also collected. Forwarding status() inherits streams and does not validate framed results. | One owned client path for product commands; any captured diagnostics are bounded/drained separately. No output()/wait_with_output() then size-check as a bounded-capture recipe. Rust unchanged. |
| Duplicate ownership and malformed error output | CLI and client both described child ownership without saying which guard; core has additional spawn/wait paths and quote-replacement JSON. | Session owns one guard; CLI supplies invocation/presentation. Typed serializer preserves error meaning. No new command-specific process manager. |
| Cross-process deadline wording was stronger than implementation | Client state creates a local deadline but serializes the original relative duration before encoding/write. Documentation spoke of one absolute deadline without specifying handoff. | Protocol distinguishes caller deadline, remaining-duration handoff, receiver frame/operation limits and cleanup. No serialized Instant, shared-clock or exact transport-delay claim. End-to-end server enforcement remains unexecuted. |

Preserved useful behavior: SocketIo retains input deadline across polling turns; Operation drops state on failed
partial I/O; NativeLocalConnection::flush is a no-op, not blocking FlushFileBuffers or a delivery receipt. These
mechanisms do not justify retaining the surrounding pairing/grant platform. Protocol AGENTS now distinguishes
one healthy terminal from none on transport failure instead of requiring impossible unconditional delivery.

The current public-client interceptor owns only serve-data-root; normal self-test reaches app/core in the primary
feature path. Native/remote-feature behavior and all other callers remain explicitly partial.

### Donors and evidence boundary

Official references read 2026-10-23:

- [Command](https://doc.rust-lang.org/std/process/struct.Command.html): output collects output and waits; status waits;
- [Child](https://doc.rust-lang.org/std/process/struct.Child.html): dropping a Child is not kill/wait;
- [process I/O](https://doc.rust-lang.org/std/process/): writing without concurrently draining the other pipe direction
  can deadlock;
- [Instant](https://doc.rust-lang.org/std/time/struct.Instant.html): opaque monotonic value, not a portable wire epoch;
- [Microsoft pipe modes](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes):
  nonblocking byte-pipe behavior does not establish an inherited-stdio implementation.

Retrieved current Rust pages identify std 1.99.1. Attempts to open version-specific 1.98.0 pages were unavailable;
no pinned-source inspection, dependency upgrade or compiler execution is claimed from those references.

No Rust/Cargo/workflow/private #598 change and no tests run. Current environment inspection found no cargo/rustc.
Git checkout failed DNS; public archive retrieval could not pass the available download route. After those
failures source work stayed on exact GitHub connector reads; it did not infer a whole-tree scan or a build.

The next static seam is the remaining native/remote/legacy CLI implementations and their real counterpart
handlers, followed by shared sealed/migration and lexical/materializer authority. A001/A013/A025 stay PARTIAL.
