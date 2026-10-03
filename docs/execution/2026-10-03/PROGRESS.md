# Integration execution — 2026-10-03

Status: **PARTIAL_PROGRESS**. The completion Goal remains active. This report is diagnostic evidence, not an issued assignment, package/API handoff, qualification receipt, gate, or wave acceptance.

The latest [restart recovery](RESTART_RECOVERY.md) revalidated Git and retained candidates, restarted
the scoped Luna Max work, and recovered the failed new enum checks without substituting historical
passes. The [qualified-profile identifier syntax](../../handoff/changes/2026-10-03-qualified-profile-id-adoption.md)
is separately accepted for pure parsing only. The reviewed [CNG primitive evidence](CNG_PRIMITIVE_DIAGNOSTIC.md)
does not qualify a persistent signing profile. The sections below retain their original capture bases.

The [pure qualified-ID parser](QUALIFIED_PROFILE_ID_REVIEW.md) is now integrated at `e88ab688` after
independent source review and exact Git-blob comparison. Its worker transcript reports the single
target passed 6/6; original stream files were not retained, and reconstructed metadata is labeled
accordingly. No profile registration or authority follows from syntax validation.

The [corrected ticket enum implementation](TICKET_ENUM_BINDINGS_REVIEW.md) is integrated at `82315138`
after independent source review: 18/18 in one combined run and structural exit 0, with original raw
captures retained. Its captured type count was 51 and issued-record count stayed zero. No passing target was
repeated after integration.

The [local bootstrap proposal](LOCAL_PROFILE_BOOTSTRAP_REVIEW.md) is integrated at `d4266f5` after
independent review for proposal publication. Its local owner pinning and actor/evidence bindings add
no Governor or Memory OS dependency. Both actual parser captures are retained with their distinct
input provenance. No key, host pin, profile qualification or issued record exists as a result.

The independently reviewed [ACK and P00 proposals](ACK_P00_PROPOSAL_REVIEW.md) are published at
`a9c1134`, with their original parser inputs and outcomes. They remain nonclaimable. A bounded native
Git object adapter is now explicitly authorized for integration-tooling implementation; activation
of issuance operations remains separate from implementing and testing that component.

The [qualified-ID registry implementation](QUALIFIED_PROFILE_ID_REGISTRY_REVIEW.md) is integrated at
`3e763ee` with all nine reviewed Git blobs preserved. Its one combined run passed 25/25 tests and its
one structural check reported 52 types and zero issued records. Original stdout/stderr and metadata
are retained. No passing command was repeated after integration.

## Delivered code

`git fetch origin` discovered that local main was 471 commits behind. A clean fast-forward moved it from `aabb12e915ba5fb7aa568756d5a37891bf1d3a11` to `9d61b759189464a01b93ca4efcb80c5398344bb4`.

- `8dc3ef19bbd128bcc2a23a2da870b7c76367bfcd`: reconciled the existing daemon tokio dependency edge in Cargo.lock and closed the P00 manifest/README/context-draft link to PUBLICATION_GUARDS_CORRECTION.md. The draft has 21 declared source files, within its 24-file ceiling, and remains non-claimable.
- `68e166ea8b327ce1d63200c504f4870955742bdd`: repaired native xtask compilation in 13 source files, preserving authority and containment checks. Independent review binds this exact integrated commit in [XTASK_REVIEW.md](XTASK_REVIEW.md).
- `f6cfb93af3b44e795e215288a086db2abb2b77f7`: aligned the structural validator and launch-state orchestration schema pin with the existing version-6 registry. Package classifications, states, and issued-record counts are unchanged. Independent source review found no blockers.
- `41feb8c876c1b54ed11a8aa56cd3f59eeac95119`: repaired W0 stage-array validation and its real-registry positive/malformed-registry negative coverage. [P00_PREFLIGHT_REVIEW.md](P00_PREFLIGHT_REVIEW.md) binds the exact integrated code commit.

All four code commits were pushed to GitHub main; remote readback confirmed `41feb8c876c1b54ed11a8aa56cd3f59eeac95119`.

The additional byte-verifier commits described below are local at this report's pre-publication
capture. Independent review binds corrected source `6e57c317233e138e178db634696cbfdc8f6bf124`;
their delivery requires a separate push and remote readback.

## Executed verification

Native toolchain: Rust/Cargo 1.98.0, Windows x64 MSVC.

| Command | Observed result | Evidence boundary |
| --- | --- | --- |
| `cargo +1.98.0 metadata --locked --offline --format-version 1` | Exit 0 after lock repair | Metadata consistency, not behavior acceptance |
| `cargo +1.98.0 check --locked -p xtask` | Exit 0, 11 existing warnings | Worker-reported run; raw transcript was not retained |
| `cargo +1.98.0 test --locked -p xtask --lib` | 109 passed, 1 failed | Pre-fix Windows fixture result; raw transcript unavailable |
| `cargo +1.98.0 test --locked -p xtask --lib context_artifact_io::tests` | 2 passed after the fixture correction | Worker-reported focused run; the full suite was not repeated |
| `cargo test --locked -p xtask --test context_artifact_builder` | 2 passed, exit 0 | Raw output retained; includes real W0 candidate and five malformed immutable-registry cases |
| `cargo +1.98.0 test --locked -p search-contracts --test conformance` | 50 passed, exit 0 | Raw output retained; owned source/tests unchanged by these integration commits |
| `cargo +1.98.0 build --release --locked -p eliot-searchd -p eliot-search --bins` | Exit 101, E0106 in search-materializer | Daemon/CLI runtime checks unavailable because build failed |
| `cargo +1.98.0 test --locked -p search-qdrant-bridge --test live_probe` | Exit 101, 83 compile errors | No live fixture or Qdrant process started from this command |
| `tools/validate-ticket-issuance-contracts.ps1 -Root <repository> -Json` | Exit 0 after schema-pin repair | Structural only: 47 types, 8 records, 211 fields, 9 signature refs, 28 workflows, 0 issued records |

Raw results are retained outside the checkout:

- `C:\Development\Rust\targets\eliot-search-luna-contracts\W0_FOUNDATION_CONFORMANCE.raw.txt`
- `C:\Development\Rust\targets\eliot-search-luna-native\evidence\native-baseline-build.txt`
- `C:\Development\Rust\targets\eliot-search-luna-native\evidence\qdrant-live-probe.txt`
- `C:\Development\Rust\targets\eliot-search-luna-preflight\evidence\ticket-issuance-contracts*.txt`

## Context preparation

The first advisory candidate invocation targeted immutable base `sha1:68e166ea8b327ce1d63200c504f4870955742bdd` and exited 2 without writing an artifact. The preflight read singular stage `phase`, while the actual W0 registry declares `phases = ["P00"]`. The narrow integration repair preserved the separate singular phase fields in ticket/context drafts.

One corrected invocation at `sha1:41feb8c876c1b54ed11a8aa56cd3f59eeac95119` exited 0 and produced candidate `4b5e219c84b1978cfa84aa05c1f710a295b2afc29f21032195b39afc49fb5489`, marked `ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED`: 21 source files, 5 exact registry fragments, 0 handoffs, all 24 preflight checks passed, 0 control-record mutations. The local bundle is 150,572 bytes, SHA-256 `f222458373cb16eb0a46b32e1be8c92c4514ef26ee86ddd99a910649e8283920`. Paths, file sizes and hashes are preserved in [context-candidate-output.json](evidence/context-candidate-output.json).

Command, runtime, stdout, stderr, and exit status are retained as `candidate.*` and `candidate-after.*` under the preflight evidence directory. The planner was not run: no real artifact-store readback, dual signatures, or canonical selection JSON exists. The candidate still declares five unavailable package checks; its structural success does not execute or qualify them. Neither an advisory candidate nor a plan creates an issued ticket or writer lease.

Independent native-dispatch inspection also found that authoritative issuance is not implemented. The current xtask exposes advisory candidate/materialization-plan/ticket-plan builders; it has no mutation runtime for `materialize_context`, `issue_assignment_ticket`, `issue_writer_lease`, or `acknowledge_writer_lease`. The schema registry says `SCHEMA_ONLY_NOT_IMPLEMENTED`, and issuance qualification is `DESIGNED_NOT_EXECUTED`. Implementing these operations requires closed-schema validation, actual signature/store verification, create-only records, exact readback, lease exclusion, idempotency, and unknown-outcome recovery. Placeholder identities/signature references cannot establish authority. A later documentation commit does not invalidate the candidate's immutable source base.

## Qdrant installation and runtime

The already installed executable `C:\Tools\Qdrant\1.19.0\qdrant.exe` matched the repository pin: 84,184,576 bytes, SHA-256 `369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4`. A live hidden process reported version 1.19.0 and commit `74f3e85b9473c62560006c043e13737ce6b48412`.

Loopback authentication checks returned 200 for the valid key and 401 for absent and incorrect keys. API keys were supplied only through child-process environment and were excluded from persisted evidence. The first harness run created the wrong literal collection due to a PowerShell interpolation error. A corrected run reached the intended collection create but received HTTP 500; the assertion discarded its error body. Its partial collection files do not establish the underlying cause. All owned processes were stopped and listeners on ports 16333–16335 were absent after cleanup.

The failed run and sanitized evidence remain at `C:\Users\kleym\AppData\Local\ELIOT\Search\Qdrant\qualification\w3-1.19.0-c4ff9869aec044249bcbef16593bc697`. After three failed harness approaches, read-only audit and the [exact v1.19.0 logging configuration](https://raw.githubusercontent.com/qdrant/qdrant/v1.19.0/config/config.yaml) were consulted. One diagnostic run preserved the exact error: `Gridstore IO error: The system cannot find the path specified. (os error 3)`. It did not identify the unresolved path.

A controlled comparison under `C:\Tools\Qdrant\1.19.0\smoke-638e27fb` preserved the executable, request body, collection-name length, and all configuration except path prefixes. Collection creation returned 200 and shard count 1. The longest partial path in the failed root was 266 characters; the longest exercised path in the successful root was 226. This supports path-prefix/path-length sensitivity; changing location and length together does not isolate MAX_PATH as the sole cause.

The existing collection then passed authentication, payload indexing, synchronous strong upsert, exact ID/payload readback, filtered query, restart persistence, and exact deletion: counts were 1 before restart, 1 after restart, and 0 after delete. The harness field-index URL/body was corrected to the [documented collection index endpoint](https://api.qdrant.tech/master/api-reference/indexes/create-field-index) and was exercised against the pinned 1.19.0 executable. The failed intermediate index-harness result is preserved alongside the successful continuation. Final independent process/listener readback found no Qdrant process and no listener on ports 16333–16335.

Portable sanitized evidence is in [qdrant-auth-crud-restart.json](evidence/qdrant-auth-crud-restart.json), [the creation/intermediate harness trace](evidence/qdrant-short-create-index-harness-failure.json), [the long-path baseline](evidence/qdrant-long-path-baseline.json), and [the server log](evidence/qdrant-server.log). Copies were checked byte-for-byte by SHA-256; the evidence folder disables Git text conversion. W3 qualification remains UNQUALIFIED: the native Rust bridge test did not compile, and the mandatory qualified client/profile/containment receipts are still absent. Qualification registries remain unchanged.

## Product and authority blockers

The [W0 independent review](../2026-10-02/W0_FOUNDATION_REVIEW.md) remains NOT_ACCEPTED: typed public-record JSON/CBOR round-trips/closed decoding are absent, and exported authorization schemas are outside the P00 contract/module registry.

Only P00/search-contracts is launch-authorized. There are no issued materialized contexts, tickets, or active writer leases. AGENTS.md requires issued ticket, exact context, active lease, writer acknowledgment, and launch/prerequisite checks before package implementation. The user directed implementation to follow the documentation; the earlier bypass clarification is no longer pending. Integration work continues through the documented procedure. No product source under `crates/**` or `bins/**` has been changed in this execution.

Concrete unapplied proposals are retained at `C:\Development\Rust\targets\eliot-search-luna-native\evidence\proposals`:

- search-materializer: one-file lifetime correction, +3/-3, binding the returned view only to manifest bytes.
- search-qdrant-bridge: eleven-file compile-only correction, +24/-24, for include-wrapper comments and visibility within `crate::live`.

Both pass read-only `git apply --check` against `8dc3ef1`; compilation and behavior of the proposals remain unverified. The manifest records exact commands, source blobs, and patch hashes.

Codebase Memory 0.9.0 indexed the repository as project `eliot-search`; scoped graph queries were executed. Native ELIOT Governor was inspected, but its default instance could not start because `C:\Users\kleym\AppData\Local\Eliot\config\governor.toml` was missing. No ELIOT verification or memory writeback is claimed. GitHub Actions remains manual-only; no workflow was dispatched.

## Standalone product direction

The user explicitly authorized assigning Luna a local-repository study and comparing retrieval results,
and required Eliot Search to run independently of Eliot Governor and Memory OS. The existing example
configuration selects the DIRECT profile and standalone instance mode. The W8 packet makes the ELIOT
adapter optional and excludes it from standalone G4 prerequisites. Development-ticket requirements are
separate from product runtime dependencies; no Governor service or Memory OS store is required or being
added for the standalone product.

A Luna Max worker studied `C:\Development\Rust\projects\eliot-swarm-controller` read-only and prepared
five same-term retrieval questions with ordinary-search anchors and per-file hashes. The configured
Cargo target directory, local target directories and PATH contained no source-matched CLI or daemon
executable at the availability observation. Eliot Search retrieval is therefore **UNAVAILABLE** for this
study; no corpus was admitted, indexed or searched with Eliot Search. Baseline findings do not establish
Search quality. The corpus contains pre-existing user changes and concurrent additions, which are
preserved; the exact anchor hashes must still match before a later comparison.

Native integration changes now target the missing issuance prerequisites. The byte verifier commit
`b2876eb348da898d090cc9da5741c1da5e6a94eb` distinguishes signed-payload and complete-file digests,
checks canonical bytes and the embedded payload digest, and delegates both prior helper consumers to
the same bounded verifier. Its focused new tests passed 8/8 and historical negative-vector coverage
passed 1/1. Review then identified acceptance of a header at byte zero, where the required preceding LF
was absent. The regression reproduced that failure; the correction at
`6e57c317233e138e178db634696cbfdc8f6bf124` passed the expanded target 9/9. Independent review of that
exact corrected source found no remaining blocker for its bounded scope, recorded in
[CONTROL_RECORD_BYTES_REVIEW.md](CONTROL_RECORD_BYTES_REVIEW.md). These checks establish no actor,
signature, complete schema, store qualification or issuance authority.

The sanitized study summary is [STANDALONE_STUDY.md](STANDALONE_STUDY.md). Raw external-repository
source/search output remains local. Six anchor blobs exist at its pinned base and six are explicitly
absent there; all twelve working-tree hashes still matched despite a concurrent corpus commit.

The bounded native issuance prerequisites now pass **17/17** focused tests at exact source
`166a4a2f59cc21998293f50071efa774220c4501`: profiles 8/8, actual-repository builder 3/3 and W0 selectors
6/6. Independent Luna review accepted that source for bounded integration publication. The
[detailed record](W0_ISSUANCE_PREREQUISITES.md) preserves earlier failures and exact captured output.
The final structural validator also reports zero issued records.

Integration decisions accept the four explicit instance-status bindings, the descriptor's root
canonical-order placement and the closed W0 selector correction. The separate accepted ticket-enum
decision adopts four vocabularies and nine field bindings; its corrected bounded implementation is
independently reviewed and verified above. Other nested enum bindings, profile/trust definitions and acknowledgement encoding
remain unresolved. The broader ticket-obligation proposal remains non-normative outside that accepted
subset. No decision creates an issued record, accepts qualification or advances launch state.
