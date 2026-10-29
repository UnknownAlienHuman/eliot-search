# Historical admission receipt v2

`eliot-admission-receipt@2` is a pre-release pure policy-provenance format. It binds policy, observation and
decision fingerprints; it does not prove native containment, a stable read, retained bytes, source identity,
currentness or access.

The current-format local application does not issue or verify this receipt. It evaluates a bounded candidate
before read, performs one stable native read, finalizes classification from the actual bytes and commits the
current source head. Storing the complete original inputs/decision/receipt is unnecessary.

The previous exact schema/domain/row order/ceilings and byte/golden guarantees are historical evidence only.
They may remain readable in a frozen diagnostic while a real old-format consumer exists. They must not:

- authorize current ingestion;
- block removal of v1/v2 admission compatibility;
- require a new migration/profile/provenance task;
- be reconstructed during normal restart;
- create a permanent package boundary or receipt ledger.

Delete this file, the receipt package and its fixtures with `search-source-provenance` and the v4 registry/
source-record archive codecs. Current behavior is specified in
[`../search-source-admission/FUNCTIONS.md`](../search-source-admission/FUNCTIONS.md).
