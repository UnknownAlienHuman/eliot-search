# #266 root admission source tranche — 2026-10-10

Base: `de07c2214097a596066d6afac308394d9b4bcd62`, accepted after #257/#342.
This tranche remains a draft implementation under open #266. It does not complete
the issue, advance the serialized Wave-2 order or qualify an installed baseline.

## Implemented source boundary

The pure `search-runtime-owner` policy distinguishes existing inspection, existing
mutation, explicit initialization and named recovery. Its decisions are proof
requirements, not filesystem or ownership capabilities.

The daemon composes distinct non-cloneable `InspectedDataRoot`, `DataRootGuard`,
`InitializingDataRoot` and initialization-recovery capabilities under existing
primary/sealed native exclusion. Stores borrow their owning capability. The admitted
layout's native handles remain retained through child lifetimes; Windows denies
share-delete, and exact installation/owner/native identities are revalidated.

Ordinary product commands use existing-only opens. Duplicate app command routes and
create-or-open DIRECT APIs have been removed from normal routing; historical create
helpers remain test-only. The current stdin service and one-shot read/mutation,
source-root, preparation and migration entrypoints use the same admission owner.
Metadata health/list construction does not resolve or create revision credentials.
Explicit source reads may resolve an existing credential, with no creation fallback.

Quarantine, pending initialization, malformed or abandoned owner state and pending
source-root registration refuse uniformly. Catalog integrity is checked before
owner succession. Child resources close before clean durable release; dropping a
guard or failing a service exchange never persists `RELEASED`.

Explicit initialization creates one empty DIRECT layout under a retained exact
intent and publishes native-bound installation format 2. Its fixed native identity
profile excludes mutable bytes/length/mtime; exact contents are validated separately.
Old format 1 is not silently upgraded. Replay only resolves a historical completion
under current clean inspection; its receipt grants no read/write capability.

Named initialization recovery requires the original complete empty layout, operation,
installation/incarnation, native root, executable and epoch-one owner token/PID and
generation. It finalizes the existing operation. Missing/corrupt partial state remains
fenced and is never recreated. The path-only legacy repair command now refuses.

The command-context continuation captures exact bounded native CLI arguments before
admission, with separate CLI/service and Windows/Unix canonical v1 domains. Request
digests use the existing canonical owner with a 512 KiB encoding ceiling; invocation
identities use qualified OS entropy. Neither identity grants root authority or supplies
missing retained recovery inputs. The private context shares one original 120-second
absolute deadline and cancellation through inspection, mutation and initialization.
Native and durable revalidation checks the context before and after observations.

Service startup has a finite context. After completed startup the same live owner may
idle; each command borrows it under its own finite context. Shutdown retains its
original command context through drain/release and final output. Migration staging
receives the original deadline. Request-aware output checks before and after I/O;
failed output signals cancellation. GC acknowledgment now stays inside its guarded
mutation closure, so output failure cannot clear quarantine or release the owner.
Native source-root arguments are intercepted before the legacy UTF-8 app parser.

DIRECT children now retain that same original request in the plaintext store;
secure-store methods delegate to it. Typed inspection, initialization/recovery and
mutation opens bind the request before catalog replay. Service dispatch rebinds a
command only after verifying pointer equality of the same borrowed live owner;
it checks the new request, allowing the service to idle beyond a previous deadline.

Cooperative checkpoints cover source traversal/ingest/publication, preparation,
secure catalog/read/search, manifest discovery/load/publication, storage inventory
and GC preview/deletion. Exact object readback receives the original absolute
deadline. Search failure branches recheck the request before creating source gaps,
and verification/search results recheck it after final construction. Cancellation
or expiry therefore returns an operation refusal instead of a complete negative.
Manifest cancellation, write and rename failures retain the attempted temporary
object. No cancellation cleanup erases partial effects or quarantine.

Administrative source-history and mapping replay now use the checked journal reader;
context-free catalog adapters are test-only. Replay checks before/after each read and
observer, and mapping checks bracket emitted/imported rows. Source-history receives
the service command's request explicitly. Its existing page limit, and directory/revision
page limits, cannot extend the original absolute deadline. Directory inventory/root/page
loops and revision-page read/preparation/result boundaries retain cancellation checks.
Initialization's empty catalog readback also uses the checked reader.

Physical revision-residue and preparation inventories retain the original request
through tree/shard/file/page loops, counts and exact fingerprint/reference reads.
Their 30-second page ceilings cannot extend the admitted deadline. Filesystem
results are captured, the original context is checked, then an ordinary error is
propagated. Classification, cursor schemas, fingerprints/preimages and file bounds
are unchanged; these read-only modes grant no repair or deletion authority.

Staging plan, content and inactive redb adapters check the bound source request
and the caller's absolute deadline through compilation, exact readback and
publication. The plan ceiling only tightens the original request. Artifact and
import API results are checked before ordinary errors propagate; mapping rows,
content revisions and BLAKE3 chunks retain cooperative checkpoints. No optional
production context, new timer or publication/recovery algorithm is introduced.

## Evidence scope

Rust 1.98.0, locked/offline, Windows native fixture profile; no dependency/lockfile
changes or automatic Actions triggers. The focused root/process target covers no-write
inspection, explicit initialization/replay/finalization, credential absence, catalog
corruption, quarantine/config-prefixed routing, native replacement/exclusion, live
layout retention, abrupt ACTIVE abandonment, clean shutdown/reopen and bounded
one-shot failure output. Pure policy has six focused cases.

Production library/binary compilation, runtime-owner strict Clippy and unchanged
canonical/Qdrant source guards are separate source gates. The complete all-target
baseline/candidate comparison uses the same features and `--keep-going -j1`, with a
separate accepted-base Cargo target. Earlier mixed-target-cache/early-cancel output is
not qualification evidence. The observed baseline and candidate harness diagnostics
remain under #189. Strict daemon compilation with warnings denied is also blocked by
the existing `search-os-secrets-windows` Clippy findings, without suppressing them.

Execution counts and the exact tested/reviewed source SHA belong in the draft PR
evidence after the final checks; this document does not infer them from source presence.
The dedicated command-context target compiles the actual private implementation and
qualified entropy owner; its strict `--no-deps` Clippy gate is a separate bounded gate,
not a claim that daemon/dependency-wide strict Clippy passed.

The dedicated child target compiles the actual maintenance, manifest codec/readback,
operation and entropy modules. Its focused cases exercise cancellation before work,
between actual orphan deletions and after manifest temporary creation, plus an actual
rename failure retaining attempted bytes. It does not qualify native admission or
durable recovery. The deletion case proves remaining-object preservation; quarantine
retention is checked by the separate product process target, not by a hand-made marker.
Public module declarations in this test binary preserve the original module visibility
without changing product visibility or suppressing fixture Clippy findings.

The native administrative-page fixture also requests the explicit orphan and
preparation-files modes on an initialized empty catalog. It proves live-owner
routing and preservation of catalog objects (excluding owner lifecycle slots)
through shutdown. It does not exercise populated physical inventories or
mid-inventory cancellation; those qualification claims remain open.

The native inactive-staging fixture uses the actual service command twice and
checks exact artifact reuse, retained technical lock and source-catalog
preservation. It compares every artifact's bytes and native identity and the
immutable text artifacts' mtimes. Inactive redb opening may change its mtime
under the existing package contract; no zero-metadata-write claim is made.
It does not qualify populated imports, mid-staging cancellation or unknown
publication recovery.

## Still open in #266 and downstream owners

- Original command checkpoints reach the named DIRECT and staging adapters above.
  Owning artifact/import library internals retain their existing deadline-only
  APIs. No production caller uses the context-free catalog replay wrappers.
  A checkpoint does not interrupt
  a synchronous OS/vendor/codec call already in progress; bounded/preemptible I/O
  and full child cancellation qualification are not claimed.
- Record-artifact Drop cleanup can erase staging evidence after an unknown
  publication outcome; #345 owns this separate lifecycle repair. The final
  locator may exist even after an error. This checkpoint slice neither changes
  cleanup nor qualifies unknown-publication recovery.
- Arbitrary legacy quarantine has no retained operation identity. Named recovery
  currently covers complete initialization only; abandoned ordinary ACTIVE/DRAINING
  and unknown migration/publication outcomes are not automatically recovered.
- Partial initialization crash recovery and the DRAINING crash point remain unqualified.
- Active redb roots refuse with `DATA_ROOT_LAYOUT_UNSUPPORTED`. redb 2.6.3's writable
  opener is not a proven non-writing/non-repairing inspection API: #343 retains this
  donor limitation. No private redb parser or silent dependency upgrade is introduced.
- Canonical provider/listener/Qdrant integration remains with its mapped owners;
  compiled optional definitions do not establish a live supported startup route.
- Native installed, full source-to-Qdrant product-spine, restart/recovery and scale
  qualification remain separate. No Qdrant or installed qualification ran here.

Donor disposition stays `PORT_CURRENT` in the Wave-2 donor map. Reuse the existing
native/control/credential owners; do not create a second root catalog or controller.
