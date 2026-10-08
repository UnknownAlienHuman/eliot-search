# Contract #48 reconciliation

**Status:** open pending normative registry edit.  
**Audited source:** `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`.  
**Owner:** #222.

The Rust implementation already uses fail-closed canonical forms for the three helpers below. The
remaining defect is documentation authority: `docs/contracts/p00/TYPE_REGISTRY.md` lacks the named
entries.

## Required exact entries

### `UtcTimestamp`

```text
canonical ASCII/UTF-8 YYYY-MM-DDTHH:MM:SS.ffffffZ
exactly six fractional digits
years 0001..9999
seconds 00..59
no alternate offset, local time, variable precision or leap second
JSON and deterministic CBOR use the canonical text
lexicographic order equals chronological order within the profile
```

### `MetadataKey`

```text
1..128 ASCII bytes
[a-z][a-z0-9_.-]*
bytewise comparison and ordering
no implicit case folding or Unicode normalization
duplicate canonical keys are invalid
```

### `UnresolvedSource`

```yaml
UnresolvedSource:
  source_id: SourceId
  reason_codes: nonempty bounded_set<SearchReasonCodeV1>
```

The record contains no display path, source bytes, free text, authorization material or evidence
payload.

## Closure rule

Close #48 only after:

1. the actual normative type registry contains all three named entries;
2. supporting schema/cross-reference links are updated;
3. no duplicate or conflicting shape exists;
4. documentation/link validation passes.

This is additive documentation closure for the existing v1 forms. It does not enable runtime,
storage, provider, product, gate or wave state.
