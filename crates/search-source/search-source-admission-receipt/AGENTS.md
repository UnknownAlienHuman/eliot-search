# Working on search-source-admission-receipt

This package is scheduled for deletion with the legacy admission provenance/archive path. Read
[README.md](README.md) and the current
[admission map](../search-source-admission/FUNCTIONS.md).

Do not add receipt fields, canonical identities, verification APIs, persisted codecs, source consumers,
compatibility wrappers or fixtures. Do not treat the old profile/domain/bytes as a current promise.

When migrating a caller, evaluate current admission directly, retain only the useful current outcome/reason
and remove the receipt/provenance dependency. Delete this package, workspace/status/ledger entries and its
old documentation after the final legacy consumer moves.

No independent implementation slice is justified for documentation spelling, byte compatibility or
qualification of a package which the current application does not use. Production verification belongs to
the real source-retention path under #270/#271.
