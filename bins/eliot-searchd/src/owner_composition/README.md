# Native root owner and explicit initialization

This daemon composition family owns native exclusion, root/installation identity and the durable
owner lifecycle. The root-open policy is supplied by `search-runtime-owner`. It does not attest a
source catalog, namespace contents, control store, revision store, security state or product readiness.
The separate initialization-only adapters verify their own initial layout and selected credential.

The first native slice exposes two explicit daemon commands:

```text
eliot-searchd --inspect-data-root ROOT
eliot-searchd --check-existing-root-owner ROOT
```

Inspection acquires both existing exclusions, pins the physical layout and reads exact installation
and both owner slots. It creates no layout and performs no explicit durable writes. The owner check
is a mutating command: it publishes ACTIVE, DRAINING and RELEASED owner records and reports the
released owner epoch/generation. Neither command opens a source or redb store. Output uses
`owner_state`; it makes no claim that the product is operational. Diagnostic output is content-free.

## Existing-only profile

Only the supported local Windows/NTFS native profile can yield a capability. Missing, unsupported,
quarantined, unresolved, corrupt or foreign state cannot become owner authority. Format-1 roots are
unsupported by this new API; the separately identified format-2 donor layout is decode-only. Normal
open never upgrades a layout. Completed format-3/journal-v2/native11 is supported without credential
acquisition during ordinary root inspection. Partial initialization, redb presence, sealed revisions
and quarantine markers remain unavailable. Named reconciliation/recovery remains separately owned.

The retained physical root and the actual primary/sealed locked handles are compared with the layout
pins before admission and on revalidation. Ten format-2 or eleven format-3 native identities bind the layout; they do not
hash catalog contents. Both owner slots must decode and form a consistent same/next-generation chain.
A corrupt newer peer cannot be ignored in favor of an older RELEASED record. Capabilities are private,
non-cloneable and retain both exclusions. Dropping one never publishes a clean release.

## Original request and limits

Explicit new initialization uses:

```text
eliot-searchd --initialize-data-root ROOT LOWERCASE_HEX_INITID
```

Exactly three native arguments are admitted. ROOT must already be an accepted empty directory; the
nonzero initialization ID is 32 lowercase hex digits. The command creates the complete initial secure
layout, purpose-bound selected revision credential and permanent
`.eliot-search-initialization-journal.v2`. It reports `initialized`/`replayed`, not product readiness.

The immutable journal keeps the original invocation ID/digest/full native argv, initialization and
installation/native bindings. Its20-field body and11-field completion use the shared canonical owner;
body/completion/journal ceilings are512KiB/32KiB/1MiB. The completion binds the existing selected
derived key and canonical historical ACTIVE/NONE, DRAINING/SHUTDOWN, RELEASED/NONE records.
Matching completed replay retains this history, verifies existing bytes/key and never creates a
credential. A new invocation has its own RequestId; original argv/digest/stable initialization ID
identify replay. Later valid catalog appends and RELEASED owner epochs are allowed.

Partial, conflicting or unknown results stay fenced for #360. No repair, recreation, unlink or
credential replacement is attempted. Initializer children/root handles close before both actual
exclusions on success, error and Drop; native resources survive terminal output. Drop writes nothing.

`NativeRootRequest` retains native argv bytes, one caller-supplied absolute `Instant` deadline and the
original shared cancellation flag. It is consumed once. It is not a capability or a durable operation
reconciliation receipt. Deadline/cancellation are checked throughout admission, before the first
possible owner-slot effect, after effects and through each terminal write/flush. Failure after a
possible durable effect or attempted success emission remains an unknown outcome; no rollback or
blind replay is claimed. Persisted request/effect reconciliation remains #348.

Positive caller-selected limits can only narrow these hard ceilings:

| Dimension | Ceiling |
|---|---:|
| Native argv count | 256 |
| Native argv input | 256 KiB |
| Complete encoded request | 512 KiB |
| Requested read-attempt bytes | 1 GiB |
| Owner I/O/ABI-observer seam calls | 16,384 |
| Opens | 256 |

Installation is at most 1 KiB, each owner slot 4 KiB and the executable 64 MiB with 64 KiB chunks.
Every short read/write attempt is charged/polled separately. The native meter includes the request's
one qualified entropy call, is never reset and gives no refunds. Initialization write reservations
share the same cumulative I/O ceiling. It bounds owner seam invocations,
not individual internal Win32 calls, CPU/RSS, filesystem access-time effects or blocking-call preemption.

## Identity and compatibility

The new request domains are `eliot/cbor/native-root-request/windows/v1` and
`eliot/cbor/native-root-request/unix/v1`, with `[mode_tag, native_argv...]` encoded by the shared
canonical encoder. They are not donor CLI-v1 bytes. The ten-object native digest keeps donor domain
`eliot/cbor/data-root-native-objects/v1`. Existing installation/owner v1 transcripts, minting and
fixtures are retained. One-shot callers still use their legacy surface until #355's separate successor cutover.
Explicit format-3 uses `legacy-direct-root-v3-journal-v2` and the separate native-objects/v2 domain.
Its persisted compatibility disposition is VERSIONED_REBUILD / explicit new initialization; old
namespace/log bytes and protection derivations are unchanged. The frozen #344 stack is not merged.

## Evidence boundary

Nine private source fixtures cover original argv/context, lower limits/native input, zero creation,
native inspection/exclusions, corrupt newer peer/barriers, pre/post-effect cancellation, admission
terminal cancellation, later lifecycle cancellation/retained poison kind and terminal emission/abandoned
Drop. They are authored, NOT_COMPILED/NOT_RUN.
The full initializer adds nine journal and nine Windows-native authored cases for bounded/rehashed
history, original request/key/replay, actual registry append/later owner release, native sharing and
release order, pre-journal foreign entry, credential-context cancellation and terminal I/O refusal.
These cases remain NOT_COMPILED/NOT_RUN; temporary roots/credentials are retained for qualification.
Late deadline expiry, replacement/foreign/moved-layout cases, injected short-I/O failures and full
lifecycle/restart/fault/resource/installed evidence remain qualification obligations under #215.
Missing/wrong credentials, ancestor rename/reparse/drive mapping, child creation under sharing and
lower native resource ceilings also require native evidence. Atomic handle-relative creation and
hard blocking-call preemption are not established by this source slice.

Production check, strict Clippy comparison and affected static guards establish source delivery only.
Inherited daemon strict-Clippy debt remains NOT_PASS under #387. Native/product qualification is not
established by a source review, this documentation, layout presence or a successful owner command.

## Existing-only read service

`--serve-data-root ROOT` now retains native read custody across startup, commands and shutdown.
Namespace/catalog bytes come from the retained pins and the unchanged strict parser/replay. The
existing protected read adapter loads only an existing credential and verifies the format3 selected
binding. Missing/foreign/corrupt/quarantined/unknown roots refuse without initialization or repair.
Sealed fallible reads keep sticky typed native failure, without mutable inner escape.

Startup retains complete native argv and original deadline/flag through ready output. Each new
command has a distinct original context/meter. Requested write attempts, short writes and flushes
consume that context; possible output loss stays unknown. Empty/prohibited command names refuse
before admission. The private ephemeral refusal profile `eliot/cbor/native-service-refusal/v1`
binds `[startup RequestId, startup native-argv digest, exact UTF8 rejected command]`, with256KiB
transport input and the existing512KiB encoded ceiling. It neither widens normal native admission
nor grants root authority. Mutations/GC/control migration remain explicitly unavailable.

Explicit shutdown retains its original request/meter/flag; EOF captures its own operation before
teardown. Search-state and store/protector children close before native exclusions. Read serving
writes no ACTIVE/DRAINING/RELEASED record and terminal output is no durable release receipt.
Fixtures for original contexts, long refusal followed by health, retained shutdown, short output,
limits and fail-stop ordering are AUTHORED_NOT_COMPILED/NOT_RUN. Native/fault/resource/installed
qualification remains #215. One-shot and mutation cutover remain #355 successors.

## One-shot existing-only read invocation — #458

The independently reviewed frozen proposal included all family oracles: command909/10,
native7303/23 and complete named closure20881/132 Rust lines/files. The current inclusive recount
is command961/10, native7396/23 and complete21040/132, including every retained/new fixture and
both product entry files. Simultaneous child ceilings are1100/7500/21300 respectively. The old
7155/22 and18915/112 measurements omitted oracles/entry closure and are superseded.
Keep <=30 production paths and2500 churn.
These are one-shot-read-only limits, not transferred service/initializer or daemon-wide allowances.

Normal one-shot health/search/list/verify/read-revision commands retain all native argv, then select
the actual root through the existing config parser. The same existing-only native read startup owns
pinned catalog/namespace and existing selected-key adoption. One consumed startup operation carries
the original deadline/flag/meter through output and child-before-exclusion teardown; no fresh shutdown
request, owner lifecycle write, credential publication or create-on-read occurs. The shared NativeOutput
adapter belongs to the sealed read family and is used by both service and one-shot output.
Legacy duplicate read handlers are test-only; mutation/source-root/preparation/control callers remain
separate #355/#356/#348 successors. Preserve old fixtures and fail-closed unsupported recovery.

The new invocation/native and changed ownership fixtures are AUTHORED_NOT_COMPILED/NOT_RUN under #215.
Final exact-head independent source review is required before merge. Production compilation and
strict baseline comparison do not establish native/runtime/installed/product acceptance.

## Catalog-only registration observation — #461

`--source-roots` consumes the existing inspection request through a catalog-only one-shot operation.
Optional registration custody retains actual read-only identity/length/mtime without adding a pin
to persisted native10/native11. Pending sidecars refuse with no recovery; missing catalog remains an
empty registration observation. Named charged metadata/canonicalize seams preserve existing gaps.
The source-root owner prepares complete64KiB-bounded ordered output. Original request/meter own
every short write/flush and final post-release checkpoint; auxiliary handle closes before exclusions.
No DIRECT startup, credentials, owner lifecycle or sync acknowledgement is reached. Mutable source
root and other normal caller families remain #355 successors. Persisted bytes remain BYTE_COMPATIBLE.

Fresh simultaneous physical Rust ceilings include every retained/new fixture and oracle:
source_roots1800(actual1678/11), native8200(actual7985/24), commands1150(actual1072/11), complete
named closure30100(actual29875/179). This is a dependency footprint, not daemon-wide write authority.
Owning fixtures remain AUTHORED_NOT_COMPILED/NOT_RUN; native sharing/absence races, no-write inventories,
late deadline/fault/resource/installed proof remain Q/#215. Source delivery is not product readiness.

## Paired registration/removal — #463

`--register-source-root ROOT SOURCE` and `--unregister-source-root ROOT SOURCE` now select the
existing native mutating root through the original native/config argv. Catalog/source/output
preflight runs under RELEASED custody before ACTIVE. Malformed arity preserves the fixed error
frame before admission; constructor and catalog refusal retain the original deadline/flag/meter.
No DIRECT store, credential, ingestion, synchronization or access authority is acquired.

The private Windows adapter uses fixed names beneath the retained control handle, synchronous
no-reparse opens and exact one-link file custody. Staged bytes are charged, synced and read back;
original-to-backup and stage-to-primary renames request no replacement. Exact primary name/identity
and bytes are checked before disposition/close of the held backup and absence readback. No pathname
unlink, rollback, recovery or blind replay occurs. Non-Windows and uncertain state refuse.

Duplicate registration preserves catalog bytes; exact stored-path removal works for a now-missing
registered root. The source owner prepares bounded unchanged v1/JSON semantics. Output is a catalog
observation, not a completed-release receipt. Every acquired owner reaches child-before-exclusion
teardown and original polling; failure never attempts further clean lifecycle writes. Native10/11,
installation/owner/source/revision/protection profiles and initialization history remain unchanged.

Exact-child simultaneous physical Rust ceilings: source2100, native9500, commands1300, complete
named closure32000, including all fixtures/oracles/adapter families and both process fixtures.
The reviewed proposal is1913/12,9370/27,1205/12,31642/184. These are inclusive footprint limits,
not daemon-wide write authority. Owning fixtures remain AUTHORED_NOT_COMPILED/NOT_RUN; native races,
sharing/hardlinks/reparse, original late expiry, I/O/teardown faults/restart/resources and installed
evidence remain Q/#215. Sync and other mutation/control/recovery paths retain their separate owners.
