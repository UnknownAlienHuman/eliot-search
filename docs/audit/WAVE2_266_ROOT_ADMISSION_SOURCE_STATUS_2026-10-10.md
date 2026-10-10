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

## Still open in #266 and downstream owners

- General command operation/deadline/cancellation bindings are incomplete. The
  initialization deadline does not qualify every operation or long-running child.
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
