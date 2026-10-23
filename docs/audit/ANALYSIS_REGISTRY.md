# Analysis registry

Audit continuation memory: inspected source, findings, changes, execution evidence and unresolved questions.
Not a gate, product specification or completion percentage. Established defect, corrected instruction,
implemented change and executed verification are distinct states.

**Original baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Latest inspected source:** `676135f49c5373b27edfd6429717464e53202974` (R024, 2026-10-23)  
**Historical qualification archive:** `2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification`

Resume from the actual PR head. Bind execution claims to exact SHA/command/target. Preserve immutable links
when condensing detailed earlier passes. Default-branch search discovers paths, not current-branch absence.

## Status vocabulary

CHECKED means the named static question, not every line/runtime property. PARTIAL means named work remains.
PENDING means insufficient inspection. RUNTIME-ONLY requires execution. PROTECTED excludes unpublished work.
The table tracks area coverage; a documented defect can remain unimplemented without becoming unread again.

## Coverage ledger

| ID | Area | Status | Established result / current state | Remaining |
|---|---|---|---|---|
| A001 | Documentation authority | PARTIAL | Historical specs removed; R023 published native/client findings. R024 resolves cancellation/terminal and donor-I/O ambiguities without a new platform. | Other owner/issue conflicts and useful historical requirements. |
| A002 | Historical governance data | CHECKED | Swarm/maps/packets/generated status removed; Git is archive. | Code remnants below. |
| A003 | Workflows/wrappers | CHECKED | Old gates/wrappers removed; Actions manual-only. | Recheck changed workflows only. |
| A004 | Workspace inventory | CHECKED for inventory | R020 removed retention member/dependency/lock shell; 50 declared members. | Actual Cargo resolution and remaining cuts. |
| A005 | Entrypoints/product | PARTIAL | R024 fixes primary CLI false command/provider advertisements. Root forwarding and native constructors still do not form indexed Search. | Connect one default CLI/daemon and verify its binary. |
| A006 | Root/config/control | PARTIAL | One root guard/typed state selected; durable build inputs differ from live snapshots. | Current state/capture and platform durability. |
| A007 | Sources/reconciliation | PARTIAL | R010 defects and live provider currentness consumer recorded. | Source/object/current-row integration and actual published freshness; #598 overlap only. |
| A008 | Private object/key work | PROTECTED | Public contract reviewed; old six-domain/purge programme not inherited. | Accepted handoff; no private branch/worktree changes. |
| A009 | Preparation/identity | PARTIAL | Test/limit identity coupling, budget/empty/stale/ABA distinctions established. | Corrections through actual callers. |
| A010 | Qdrant/donors | CHECKED for named static questions | Gate removed; BM25/tokenizer/IDF/readback/late-write limits recorded. | Real connection and relevance/scale/late-write execution. |
| A011 | Publication/view/cleanup | PARTIAL | Two-sided reconciliation, immutable retrieval and captured inputs selected. | Crash/view/rebuild/cleanup implementation. |
| A012 | Query/evidence/window | PARTIAL | Earlier evidence/window defects recorded. R024 stages window allocation/arithmetic before output; late cancellation cannot rewrite a started terminal. | Actual query/window integration and remaining callers. |
| A013 | Protocol/client/CLI | PARTIAL | R024 closes the named CLI transport/response/cancellation helper readset; finds pre-byte timer origin and distinguishes useful partial-frame state. | Remaining daemon handshake/work/native ownership closure and real session cutover. |
| A014 | Product depth | PARTIAL | Exact/navigation/comparison/documents/models/MCP remain requirements. | Concrete producers/consumers and quality assessment. |
| A015 | Tests/governance code | PARTIAL | R024 reads native fixtures and remaining CLI test; response-named test actually checks a request envelope, not a result. Useful budget/no-effect cases retained. | Mixed migration and other structural consumers; no broad test deletion. |
| A016 | Donors | PARTIAL | R024 reads Tokio 1.53.1 process/Child/select APIs and exact stdin/read_exact/write_all source. Parent async pipes selected; console/process-exit limitations explicit. | Only concrete remaining seams; platform timing unexecuted. |
| A017 | Qualification tree | CHECKED | Archived; policy/blocker notes remain. | Runtime ceremony A022. |
| A018 | Auxiliary documents | CHECKED for filename/disposition | Duplicate supplements removed; retention path is notes only. | Semantic consistency A001. |
| A019 | Issues/PR handoff | PARTIAL | GitHub write APIs confirmed; R023 published as 676135f4. R024 updates affected issue/PR references. | Other owners as inspected; no whole-queue claim. |
| A020 | Runtime evidence | RUNTIME-ONLY | Static counterexamples and deleted fixtures are not platform/recovery/performance evidence. | Exact-SHA runs after connected implementation. |
| A021 | Compiler baseline | PARTIAL | Previous bridge-lib evidence: run 39872058276 at 7d92fb8b only. R024 CLI Clippy attempt stops at cargo not found, exit 127. | Locked metadata and actual CLI/daemon/current consumers; Windows separately. |
| A022 | Runtime qualification/identity | PARTIAL | Qdrant activation gone; lexical/materializer fixture authority remains. | Real caller cleanup; causal examples stay ordinary tests. |
| A023 | GC/storage diagnostics | PARTIAL | Generic platform/shell gone; typed collector and query inventory removal unfinished. | Stopped-root collection and no hot-path deep inventory. |
| A024 | Backup/restore | PARTIAL | Fresh archive/destination design recorded; old generic restore removed. | Implementation and partial/corrupt/interrupted/protected behavior. |
| A025 | Features/harness topology | PARTIAL | Native/legacy CLI constructor boundaries mapped. R024 inspects historical daemon canonical I/O/serving, not whole feature closure. | Wave1–wave6 and remaining counterpart/consumer map before Cargo pruning. |

## Remaining static audit

1. Daemon canonical `tcp/handshake.rs`, `tcp/serving/work.rs`, `provider_composition/local_stream.rs` and native
   bootstrap/authority producers/callers; complete current-branch wave1–wave6 and shared sealed/migration closure.
   R024 has read CLI core transport/response/cancellation and named native tails/fixtures; do not list them as unread.
2. Reconcile remaining local documents/issues against actual code and preserved useful requirements. No new global
   specification or package-per-noun interpretation.
3. Lexical/materializer fixture activation and configuration-authority consumers.
4. Mixed migration tests: move applicable filesystem behavior, remove source-shape assertions with old owners.
5. Concrete exact/navigation/comparison/document/model/MCP producer/donor seams.

Known implementation is separate: connect source/object/current-state/preparation, Qdrant/publication and
query/evidence/window/session; apply recorded empty/gap/stale/lost-wakeup fixes; implement current GC/import.
Deleting scaffolding or correcting help is not delivery. Runtime claims require the relevant connected path.

## Immutable detailed evidence

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
| R021 | [ff59f930](https://github.com/UnknownAlienHuman/eliot-search/blob/ff59f930c5b160b1ed36684538be3dca1bdd6970/docs/audit/ANALYSIS_REGISTRY.md) |
| R022 | [305ccab1](https://github.com/UnknownAlienHuman/eliot-search/blob/305ccab141774dba088e38dacc006fc90fcfb0a2/docs/audit/ANALYSIS_REGISTRY.md) |
| R023 / previous ledger | [676135f4](https://github.com/UnknownAlienHuman/eliot-search/blob/676135f49c5373b27edfd6429717464e53202974/docs/audit/ANALYSIS_REGISTRY.md) |

## R024 — transport tails, actual timer origin and donor cancellation (2026-10-23)

### Inspected source

Source head `676135f49c5373b27edfd6429717464e53202974` (only R023 Markdown changes after 305ccab1):

- root implementation guide; current CLI/client/protocol recipes and R023 record;
- CLI `src/provider_client/core/{transport,response}.rs`, `core/exchange/cancellation.rs`, complete files;
- CLI `src/provider_client/core.rs`, line 610 through EOF (remaining helpers/tests);
- CLI `src/native_local_registered/tests.rs` and `tests/legacy_effects_refusal.rs`, complete files;
- client `src/typed/native.rs` line 500 through EOF; `src/typed/state/grant.rs` line 225 through EOF;
- client `src/typed/native/expectations_tests.rs`, complete file;
- daemon `src/provider_composition/kernel/canonical/tcp.rs`: construction, record admission, delivery and closure
  (first response plus overlapping line 350–EOF read); its `tcp/{io,serving}.rs`, complete files;
- primary CLI `src/app.rs` exact blob 96db9ebd012c30f344d2f51dc7cef93918219d0b reconstructed locally and verified;
  R022/R023 retain its actual entry/core dispatch evidence.

No whole-tree checkout, feature/caller absence proof, donor upgrade, private work or subagent execution is claimed.

### Findings / dispositions

| Finding | Evidence | Action / remaining limit |
|---|---|---|
| Useful old partial-frame behavior | CLI ExchangeBudget retains partial, line_limit, bytes and lines when begin_cancel_cleanup changes only the finite cleanup deadline. DeadlineCancellation is restricted to Health/Version. | Preserve positions/counters and same reader; do not reset the parser or infer general Search cancellation from this diagnostic helper. Old code is not removed by R024. |
| Idle time charged to a future request | Daemon SocketIo::poll_record creates Incoming.started before any successful read. CanonicalTcpConnection later uses it as admission clock; serving polls even with an empty queue. | Separate idle from first-byte/frame and request clocks. With a sufficiently large server cap, 20 seconds idle followed by a 1-second request can already exhaust that request on arrival. Static counterexample, not an executed default-product failure. |
| Poll quantum does not bound output | CanonicalServingOwner services input/work in turns, but its output calls synchronous write_parts until whole record/flush/deadline. | One queue/quantum label is not responsive duplex I/O. Do not copy the round-robin/authority platform into the one-active-request replacement. |
| Test title overstates covered behavior | core.rs response_receipt_verification_is_tamper_evident builds a request via seal_envelope and calls verify_envelope_proof; it never calls verify_sealed_response or validates printed payload. | Do not count it as reply/result verification. Delete its obsolete protocol obligation with that owner; preserve actual DTO/evidence behavior in the connected path later. No test executed. |
| Replacement terminal rule was ambiguous | R022 allowed live cancellation but only said commit after flush; it also collapsed pre-output cancellation and broken framing into session failure. A late Cancel or fallible insertion could conflict with an already advertised terminal. | Protocol now distinguishes before-first-byte, writing and completed output; no second terminal; prepare allocation/arithmetic before output and perform only local infallible commit afterward. Ordinary request outcome differs from damaged transport. Design correction, not runtime fix. |
| Async rewrite could lose frame state | Tokio ReadExact owns ReadBuf progress; WriteAll advances its private remaining slice. Dropping/recreating these futures in a select loop loses that progress. | Select cancel-safe read/write with positions retained in the session. Abandon partial state only when discarding the connection. No custom scheduler or new codec platform. |
| Async stdin is not cancellable stdin | Exact Tokio stdin source uses Blocking<std::io::Stdin> and explicitly warns that cancellation cannot stop its read. | Parent uses Tokio child pipes; standalone console input has one process-lifetime reader and explicit exit policy. Daemon inherited input retains owned-child termination fallback. No detached per-timeout tasks or hard platform timing claim. |
| False primary help is actual code, not only documentation | R023's two advertised commands lack dispatch arms; native declarations do not establish a provider. Health/shutdown spawn a new shell child. | R024 changes only CLI app.rs comment/help: removes phantom commands, labels old forwarding and states actual scope. Command routing, old feature-only help and transport remain unchanged. |

Read fixtures are not all useless: native_local_registered checks propagation of the original deadline and
pre-cancellation before credential access. Those causal properties survive without its registration types.
NativeClientExpectations fixtures explicitly declare synthetic/non-native scope. legacy_effects_refusal exercises
local refusal before spawn; keep that behavior while replacing the obsolete command surface. Do not delete useful
behavior merely because nearby tests freeze an old wire spelling.

### Donors

Exact pinned Tokio 1.53.1 sources read through GitHub:

- `tokio/src/io/stdin.rs`, blob 877c48b30fbe12d7c99bab71a52a3a9b5ba9573d;
- `tokio/src/io/util/read_exact.rs`, blob e9e5afbf0b9b2ac45d47406a1c6fcbdb9eaf64c1;
- `tokio/src/io/util/write_all.rs`, blob 8330d89e1fcd0741ce6c161bded9fbfaf999228f.

The client recipe links these sources and exact-version process/Child/select API docs. read/write cancellation
safety, process kill/wait distinctions and asynchronous child-pipe ownership are mechanisms, not proof that this
checkout implements them. Two unavailable stdin documentation pages were replaced by the exact tagged source,
not repeatedly retried or silently replaced with another version. No Cargo/dependency change in this pass.

### Published scope and verification

R023 is published as 676135f4 using the GitHub Git-data write actions after full catalog reload and push permission
check. This pass adds a separate R024 commit: CLI help Rust plus CLI/client/protocol instructions and this register.
No workflow, authentication/storage/private #598 code or tests changed. New transport rules are unimplemented.

The CLI source reconstruction matched the original Git blob; the selected code blob is
448ff8d2f7de9b20f6fd8f7a34828c295ff47959. Its diff changes only comments/help, not routing or literals used by error
handling. Static diff validation is not compilation. Attempted command:

```text
cargo +1.98.0 clippy --locked -p eliot-search --bin eliot-search -- -D warnings
```

The shell returned cargo: command not found (127), before any compilation. No new successful compiler, test,
transport, timer, terminal-race or platform run is claimed. Last historical bridge-library evidence does not
cover this head. GitHub publication uses expected-head non-forced updates; local network/DNS failure does not
prevent those API writes. A001/A013/A025 remain PARTIAL with the explicit smaller unread set above.
