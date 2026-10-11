# Existing native root owner

This daemon composition family owns native exclusion, root/installation identity and the durable
owner lifecycle. The root-open policy is supplied by `search-runtime-owner`. It does not attest a
source catalog, namespace contents, control store, revision store, security state or product readiness.

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
open never upgrades a layout. Redb presence, sealed revisions and initialization/quarantine markers
remain unavailable. Explicit initialization and named recovery have separate children under #354.

The retained physical root and the actual primary/sealed locked handles are compared with the layout
pins before admission and on revalidation. Ten ordered native identities bind the layout; they do not
hash catalog contents. Both owner slots must decode and form a consistent same/next-generation chain.
A corrupt newer peer cannot be ignored in favor of an older RELEASED record. Capabilities are private,
non-cloneable and retain both exclusions. Dropping one never publishes a clean release.

## Original request and limits

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
one qualified entropy call, is never reset and gives no refunds. It bounds owner seam invocations,
not individual internal Win32 calls, CPU/RSS, filesystem access-time effects or blocking-call preemption.

## Identity and compatibility

The new request domains are `eliot/cbor/native-root-request/windows/v1` and
`eliot/cbor/native-root-request/unix/v1`, with `[mode_tag, native_argv...]` encoded by the shared
canonical encoder. They are not donor CLI-v1 bytes. The ten-object native digest keeps donor domain
`eliot/cbor/data-root-native-objects/v1`. Existing installation/owner v1 transcripts, minting and
fixtures are retained. Normal callers still use their legacy surface until #355's separate cutover.

## Evidence boundary

Eight private source fixtures cover original argv/context, lower limits/native input, zero creation,
native inspection/exclusions, corrupt newer peer/barriers, pre/post-effect cancellation, admission
terminal cancellation and terminal emission/abandoned Drop. They are authored, NOT_COMPILED/NOT_RUN.
Late deadline expiry, replacement/foreign/moved-layout cases, injected short-I/O failures and full
lifecycle/restart/fault/resource/installed evidence remain qualification obligations under #215.

Production check, strict Clippy comparison and affected static guards establish source delivery only.
Inherited daemon strict-Clippy debt remains NOT_PASS under #387. Native/product qualification is not
established by a source review, this documentation, layout presence or a successful owner command.
