# search-source-admission-receipt — historical removal leaf

This 34.4 KiB pure receipt package exists to preserve the pre-release v2 admission decision/fingerprint
profile. It does not read a source, prove native containment, retain bytes or commit current state.

Its production consumer is the legacy `search-source-provenance` archive/reconstitution path. The current
local ingestion target evaluates the selected candidate directly and stores only the useful current outcome.
It needs no separately issued or verified admission receipt.

## Disposition

Delete this package with:

- `search-source-provenance`;
- source-record/registry v4 original-input archive codecs;
- `OriginalAdmissionV2` from native acquisition;
- receipt/profile/golden fixtures and documentation;
- workspace/dependency/status/ledger entries which exist only for this leaf.

Do not preserve the `eliot-admission-receipt@2` bytes, API path or canonical fingerprint as compatibility.
Unsupported pre-release roots are refused before writes and left intact. A historical diagnostic may keep a
read-only copy only while it has an explicit real caller; it cannot participate in current ingestion.

The current admission flow is described in
[`search-source-admission/FUNCTIONS.md`](../search-source-admission/FUNCTIONS.md) and issue
[#270](https://github.com/UnknownAlienHuman/eliot-search/issues/270).
