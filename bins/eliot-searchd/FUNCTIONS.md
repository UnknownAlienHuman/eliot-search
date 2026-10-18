# Function contract — `eliot-searchd`

Fresh DIRECT initialization creates one fixed schema-5 `control/control.redb`
under retained Native root/parent/database custody. Normal Root registration and
listing use this same journal, publisher and atomic semantic registry protocol.
Actual native identity binds the expected journal; stored headers, locator hints
or marker presence cannot grant admission. Known prior/current owner epochs are
verified before consuming succession; missing, corrupt, foreign or Inert state
never creates a replacement or falls back to a legacy authority.

The optional file-only `source_root_locators.entries` maps RootBindingId to a
lossless UTF-8 locator hint. Actual FD identity/current permissions and database
state remain authoritative. Historical operation replay does not reactivate an
Unbound Root. The explicit bootstrap-v1 policy selection is Root metadata only;
it is not current permanent-v2 Source admission. Metadata output keeps
`ready:false`, source/service/query consumers unavailable and no fabricated live
read-security fence. Original request/work/unknown outcome and ordered teardown
remain load-bearing. Native and installed product qualification remain separate.
The separate `--register-source-root-v2 DATA_ROOT SOURCE_ROOT POLICY_ARCHIVE`
restores the closed canonical `RetainedPolicyV2` archive before mutating owner
acquisition. The same original request and work meter own qualified read-only
metadata capture. Root registration uses the actual compiled v2 revision and
fingerprint; changed policy/profile never upgrades or reactivates an old Root.
This remains Root metadata only. First Source admission owns namespace creation
and durable original-policy reference binding; an empty Root has no archive store.
The shared Native journal identity recipe is compiled in the default DIRECT
profile; it does not require the optional query/provider feature or Qdrant.

`--admit-source-v2 DATA_ROOT SOURCE_ROOT RELATIVE_FILE POLICY_ARCHIVE OPERATION_ID EXPECTED_SOURCE_REVISION`
admits one regular NTFS file under that registered v2 Root. The operation ID is
32 nonzero lowercase hex digits; the predecessor is `new` or a positive Source
record revision. Metadata admission precedes body reads; one retained synchronous
read-only/no-follow FD supplies stable bytes and measured C04 identity. Final
admission and Source/namespace originals commit through the same redb authority.
Exact operation replay precedes a fresh Source open and preserves the original
receipt; changed intent conflicts. Output is Source-admission metadata only:
`retained_revision:false`, `source_body_serving:false`, `ready:false`. Immutable
revision retention, ingestion batches and indexed query serving remain separate.

`--prepare-revision-dek-v1 DATA_ROOT OPERATION_ID` prepares one fresh revision
DEK candidate. OPID is a stable nonzero 16-byte lowercase hex nonce. Actual
Windows user observation, Native root custody and the original invocation
context bind RNG, current-user DPAPI and exact Credential Manager publication.
The same redb retains Intent before publication and conditionally records
CandidateNonServing only after protected-key and control readback. Current must
remain absent. Replay looks up the retained operation before key preparation;
unknown/missing/contradictory state never creates a replacement. This command
issues no Current selection, key-use capability, revision encryption or serving
readiness. Fixture targets are captured before possible writes and deleted by
exact reference with verified absence on every teardown path.

Native executable verification hashes fresh bytes at every full fence. The
native caller uses admitted heap scratch up to 1 MiB and the actual opened length,
charges each requested read/short-read attempt, and verifies one-byte EOF before
the unchanged post-read metadata and fresh-locator/native-identity comparison.
The 64 MiB image bound, two opens, original context and owner-executable v1 BLAKE3
transcript remain; the legacy reader is unchanged. This reduces owner I/O seam
reservations without caching the image or changing a resource ceiling.

Pinned revision readers obtain full native observation and live link count from
one file-information result, with original pre/post callbacks for every ABI.
Each before/after read boundary queries the retained FD afresh and still checks
single-link admission, metadata, current path access and final locator identity.
The legacy observation and hardlink APIs remain compatible. This removes a
duplicate probe; it does not cache source bytes, permissions or currentness.

The offline `--stage-source-control-migration ROOT TARGET_UUID OUTPUT_DIRECTORY`
route keeps the original native read operation, registration/protector custody,
deadline, allowance and output pins. It composes completed bootstrap inventory,
two-pass source-map publication, same-database mapping/inventory adoption and
original-bound source-content publication, import and full readback.
The owning partial receipt reports complete_semantic_candidate_not_performed;
this route grants no serving, candidate, cutover, restart or cleanup authority.

Content and inventory share two independent source-backed detailed-read passes,
using the frozen source-content v1 and location-inventory v2 encoders. Each current
revision's final representations stay pinned through readback and physical-row
emission. Inventory reuses their exact raw SHA/length/native-identity observations;
plaintext is released before the next revision. Import and independent target
readback consume fresh verified cursors over the retained immutable artifacts.
Temporary/orphan residue stays explicitly accounted against the complete physical denominator.
The original request supplies all native and record-artifact work; no new deadline,
directory, source inventory or digest cache is created. A linear traversal of
canonical revision occurrences joins exact legacy source/revision references to
the imported SHA/length/BLAKE3 facts. Reused legacy objects do not collapse A/B/A
occurrences. Missing historical content remains an explicit gap; wrong bindings,
orphan content and unknown publication cannot issue a partial success receipt.
The final CLI filter admits only exact content phase/outcome codes and recognized
literal cause suffixes. An unrecognized/private cause is collapsed while preserving
the public content outcome; it cannot inject source paths or JSON frame contents.

Contextual legacy unprotect uses the original startup request/work pair or the
separately admitted command pair for mandatory native-clear accounting, including
after ordinary work refuses. The protector admits its final validated plaintext
capacity and copy/zeroization work; revision callers do not reserve that same
capacity again. The revision and preparation consumers share this guarantee.

Inventory directories use one pinned locator snapshot, capped at 2,048 names,
with canonically ordered 32-name output windows. Geometric buffer growth,
index scratch, comparison bytes and linear permutation work are admitted before
use. Explicit merge ordering has at most `n * ceil(log2(n))` key comparisons.
The directory is reobserved after the visitors, not only after enumeration.

The native producer selects eliot.legacy-inventory.v2. Physical subjects bind
root-relative native component bytes, including tree, parent and shard, with
length framing and a platform tag. Successful directory observations and decoded
preparation reference/key pairs retain that binding. The SHA-256 locator is
correlation only and grants no source, deletion or cutover authority. V1 evidence
keeps its original basename interpretation in the owning codec; publication and
same-target adoption retain the exact selected profile. Absolute paths and
arbitrary bodies do not enter these metadata rows.

One pass-local preparation link view is shared by Preparation and Residue;
the second complete stream independently rebuilds it. Construction reads each
retained reference and its target at most once; physical lookups do not reopen
them. Unknown preparation-root files remain Unsupported, directories remain Gap,
and bodies of unknown entries are never opened or recursively traversed.

Preparation and revision traversal retain pass-local NTFS parent custody, bounded
by eight ancestors and 2,048 directory pins. Native direct-child membership uses
the held directory FD and caller-admitted 64 KiB scratch; incomplete enumeration
never supplies membership. The opaque directory qualifier retains the same FD.
Warm identity rechecks freshly observe volume, full index, creation time and
directory/reparse attributes inside one bounded original-context bracket.
This does not cache access rights: full-path leaf READ and directory LIST/traverse,
single-link/EOF/raw-byte joins and final locator observations remain independent.

A completed revision traversal retains only ordered scalar residue tuples and
parent custody until that source pass ends. Referenced final representations,
temporary/orphan/unknown/corrupt rows and directory outcomes keep their existing
family dispositions. No body is retained, partial capture never installs a view,
and the independent next source pass reconstructs both custody and observations.

The original whole-command hard ceilings remain 1 GiB logical byte work,
16,384 admitted native I/O seams, 256 opens and the original 120-second deadline. Per-directory
and link-cache caps are additional refusal boundaries, not a guarantee that every
shape below those caps fits the whole command. Native/scale qualification is #215.

The explicit offline bootstrap command composes the `search-control-redb`
original-operation bootstrap and inventory producer under one retained native
read child. The output pins and target are external/inactive; source custody is
never relabelled ACTIVE. Only the package-owned first-phase receipt is emitted.
No normal serving, complete candidate or cleanup authority follows; native,
restart/fault and installed qualification remain separate.

## Composition responsibilities

The source boundaries above are implemented but do not establish complete or installed readiness.
The daemon is the sole composition root; capability behavior remains in the existing libraries.
The following are behavior obligations, not a required builder API or a new package/profile registry.
Historical W1–W10 handoff status and review receipts are not runtime inputs.

| Responsibility | Required boundary |
|---|---|
| Composition | Construct the concrete adapters needed by the supported profile and bind one implementation per required port. Keep vendor/native handles private and dependency direction toward the existing owners; CLI/workers/leaf adapters have no direct store path. |
| Startup | Acquire the exact data-root/incarnation owner before secrets, journal or endpoints. Conflicting or ambiguous ownership blocks/quarantines. Open/verify the exact control journal under that guard; explicit migration uses its owner contract. Reconstruct the committed immutable snapshot before admission. Committed-but-unpublished state remains recoverable and fail closed. |
| Configuration | Capture bounded inputs/provenance outside pure `search-config`; reject unknown prefixed keys or plaintext-secret-shaped inputs. Owner validation rejects the whole invalid candidate. Preserve every security barrier, restart, drain, rebuild, route/publication and reclaim obligation in the composite activation plan. |
| Activation and recovery | Keep the old fingerprint/snapshot authoritative until all required effects and guarded control/snapshot publication succeed. Otherwise retain it or explicitly fail closed/quarantine the affected capability. Recover uncertain mutations through exact operation identity and owning-package readback; never blindly retry the entire plan or fabricate rollback. |
| Readiness | Derive one coherent snapshot from actual config, handlers, control generation, routes and live prerequisites. Exact artifact/profile/settings qualification still governs the operations that require it; package presence, feature selection, endpoint response and Qdrant health alone establish none of it. Keep truthful DIRECT degradation when its own prerequisites permit. |
| Requests | Require mutual pairing, current binding/grant, incarnation/session sequence, a supported handler and finite connection/frame/in-flight/backpressure/deadline budgets. Admit against one immutable control/capability snapshot with zero hot-path redb writes. Dispatch once, preserve monotonic progress and at most one terminal result, including partial/degraded/ambiguous outcomes. |
| Cancellation and disconnect | Stop emission/admission as applicable and release request-local handles, continuations, pins, windows, quotas and leases. A possible durable mutation remains the owning package's explicit unknown outcome; disconnect does not delete its recovery obligation. |
| Drain and shutdown | Stop admission, finish/cancel bounded requests and stop services in reverse dependency order. Release the data-root owner only after exact cleanup evidence proves no service still requires it. Unknown durable work or cleanup blocks unsafe release; daemon crash recovery starts from actual root/control/process/operation facts. |

Purpose/incarnation-bound secret leases and concrete handles are non-serializable/private. Stop their
consuming services before releasing those resources. Plaintext secrets stay out of config, environment, argv, logs, errors, receipts and
process launch. Normal startup never downloads, upgrades or silently selects an artifact/provider.
Optional and evaluation modes require their actual explicit admission; mere source/config presence
cannot enable them, and evaluation grants no direct-store bypass.

### Readiness and client boundary

Readiness distinguishes `STARTING`, `DIRECT_READY`, `INDEXED_READY`, `DEGRADED`, `DRAINING`,
`QUARANTINED` and `STOPPED`, with exact exclusions/reasons. A recipe is available only when its real
handler and every required operational prerequisite are live. Binding-filtered descriptors narrow
disclosure; they do not grant access. Recover a lost capability-publication receipt by exact snapshot
generation/digest readback. Never infer source currentness or indexed completeness from Qdrant.

The native local listener seam in
[StandaloneLocalOwner](src/access_composition/native_binding_listener/local.rs) derives an endpoint
from finalized registration and accepts a platform factory. At the reviewed source `c8e38b6`, it has
no concrete `StandaloneLocalListener` implementation/caller. Existing pipe connection and session
owners are separate delivered code, not a completed listener/CLI path. See the
[CLI cutover map](../eliot-search/FUNCTIONS.md#current-source-and-missing-linkage) and #332.

### Failure and qualification evidence

Preserve the typed failure distinctions: invalid/unavailable profile or dependency; uncompiled feature;
startup failure versus unknown startup outcome; owner/control unavailable; adapter binding or capability
incoherence; config failure versus unknown activation; unauthorized/resource-exhausted/cancelled request;
dependency degradation; security fail-closed/quarantine; incomplete shutdown and blocked owner release.
Errors remain content-free; one generic failure must not erase a recovery obligation.

Keep causal startup/config/capability/shutdown crash boundaries, second-owner refusal, secret redaction,
snapshot-before-admission, handler/route coherence, truthful DIRECT degradation, bounded request cleanup,
zero hot-read writes, exact unknown-outcome recovery and safe owner release in the evidence inventory.
Pure composition seams and Windows/redb/Qdrant/provider qualification prove different things.
The [execution protocol](../../docs/product/EXECUTION_PROTOCOL.md#4-implement-then-verify-the-production-closure)
selects production targets/guards and defers test/native/fault/scale execution until integration;
historical wave, dependency-map or line-budget checks are not new product admission gates.
