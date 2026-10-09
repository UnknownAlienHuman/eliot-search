# Contract #48 reconciliation

**Status:** resolved in the active normative registry.  
**Verified source:** `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`.  
**Issue:** #48 closed as completed on 2026-10-08.

The active `docs/contracts/p00/TYPE_REGISTRY.md` already contains exact named entries for:

```text
UtcTimestamp
MetadataKey
UnresolvedSource
```

## Verified closure

### `UtcTimestamp`

The registry fixes the 27-byte ASCII/UTF-8 form `YYYY-MM-DDTHH:MM:SS.ffffffZ`, six fractional
digits, years `0001..9999`, seconds `00..59`, UTC `Z` only, valid Gregorian dates, canonical JSON/CBOR
text and bytewise chronological ordering. Alternate offsets, variable precision and leap seconds are
rejected.

### `MetadataKey`

The registry fixes 1..128 ASCII bytes matching `[a-z][a-z0-9_.-]*`, bytewise ordering, canonical
uniqueness, no implicit case folding/Unicode normalization and no authority/content semantics.

### `UnresolvedSource`

```yaml
UnresolvedSource:
  source_id: SourceId
  reason_codes: bounded_set<SearchReasonCodeV1> # non-empty, max 64
```

The object is closed and carries no display path, source bytes, free text, authorization material or
evidence payload.

## Correction

The previous reconciliation note said the type registry lacked these entries. That note was stale and
contradicted the active normative file. This document records the verified state; it does not change
wire bytes, enable runtime behavior or claim product qualification.
