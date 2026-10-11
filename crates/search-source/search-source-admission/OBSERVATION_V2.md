# OBSERVATION_V2

## Purpose and boundary

`AdmissionRelativeLocator` and `AdmissionObservationV2` are a pure byte vocabulary and an
immutable equality model. They are not admission authority. They create no decision, policy,
receipt, batch, matcher, or native-path result, and they supply no v2-to-v1 fallback.

A relative locator classifies bytes only. It proves no containment, source identity, root
registration, access, currentness or native name resolution. An exact unavailable locator is
data; it never implies a match or an allow. Evaluating locator gaps is a later owner's work.

`CanonicalRelativePath` in search-source-identity at current.rs:42 is the existing canonical
relative identity and is unchanged here. `RelativePathToken` is bounded text only and is never
native final-handle proof. This module adds an admission-owned frozen profile, not a duplicate
of that type.

## Frozen locator profile

Profile id `eliot-admission-relative-utf8-portable@1` is immutable and is bound into every
fingerprint, so a future profile change is a new identity rather than a silent relabel.

| Bound | Value |
| --- | --- |
| Total locator bytes | 4,096 |
| Bytes per component | 255 |
| Components | 128 |
| Complete preimage bytes | 16,384 |

`AdmissionRelativeLocator::from_bytes(&[u8])` validates a borrowed slice and only then copies.
UTF-8 is validated before validation continues. Case and Unicode bytes are preserved exactly: no
normalization, no `Path` conversion, no lossy conversion.

Rejected, never normalized: empty input; over-limit totals, components or component bytes;
leading or trailing slash; empty, `.` and `..` components; backslash, colon, NUL and Unicode
control characters; Windows-forbidden `<>"|*?`; trailing `.` or space in a component; and
case-insensitive DOS device basenames CON, PRN, AUX, NUL, CONIN$, CONOUT$, CLOCK$, COM1-9 and
LPT1-9, including superscript `¹²³` aliases. The basename is split before the first dot and
trimmed of ASCII spaces at both ends before the alias check. Every rejection returns the content-free
`ADMISSION_OBSERVATION_INVALID`; no raw bytes, coordinates or path appear in the error or in
`Debug`.

`Debug` reports the profile, the byte count and the component count only.

## Equality model

`AdmissionObservationV2::new(metadata, relative_locator)` owns validated inputs and computes
`ObservationFingerprintV2` through the shared `blake3_canonical_array` under domain
`eliot/cbor/admission/observation/v2` with schema version 2 and the fixed 16 KiB preimage
ceiling. There is no local CBOR writer, no copied hash, no caller-supplied fingerprint and no
public digest constructor.

Fifteen ordered canonical rows, index-fixed:

0. header `[schema, profile, total-bytes, component-bytes, components, preimage-ceiling]`
1. locator class
2. location class
3. source kind
4. source class
5. optional byte size, absent becomes canonical null
6-9. optional generated, vendor, binary, system flags
10-12. optional sensitivity, detector id, profile id
13. sorted unavailable-field text array
14. optional locator raw byte string, absent becomes canonical null

Metadata arrives through its existing sole validator, so it is already opaque and
constructor-bounded. Rows are built lazily from bounded fields, with no retained row vector and
no complete preimage. `as_bytes` and `relative_locator` are read-only accessors; a caller
cannot mutate the retained model or supply its fingerprint.

The metadata(), relative_locator(), fingerprint(), component_count() and as_bytes() getters expose immutable data. Equality grants no admission, native-path, source-revision or completeness authority.

## Compatibility and consumer boundary

VERSIONED_REBUILD if and only if consumers adopt v2. Current v1 is BYTE_COMPATIBLE and
unchanged: v1 observation types, preimages, callers, `rule.pattern` retention and the retained
v1 SHA-256 writers are untouched. There is no migration window, no cutover and no v2 operand
shape in this slice.

Reverse consumers, the registry and reconcile libraries and the daemon binary, keep their
existing v1 calls and signatures and must compile. No adapter, manifest, lockfile, workflow,
root, source-identity, safe-reader or shared-source change is introduced, and no dependency or
donor is added.

## Gates and qualification

Locked offline Rust 1.98 owner check and full strict, immediate
consumer check and strict, and the daemon check with an exact 375-diagnostic comparison against
D/#387, which remains NOT_PASS, are scoped source gates. Canonical, Qdrant, path, retained-writer,
API, manifest-lock, ledger, diff and status guards apply.

Private fixtures covering accepted byte-preserving Unicode, malformed or traversal or native or
alias rejection, exact and plus-one per dimension, deterministic and differentiating identity,
missing locator and metadata and profile and domain schema separation, complete metadata and
relative encoding, and Debug and error non-disclosure are AUTHORED_NOT_RUN under #352, #241 and
#215. No all-target, test, runtime, native, installed, Qdrant or scale execution is claimed.
Integration 267, 270 and 271, runtime 333 and qualification 215 are separate owners.

Full #241, the programme Goal, Wave2 and #235 remain incomplete.
