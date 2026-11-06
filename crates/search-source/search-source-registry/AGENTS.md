# Working on current source values

Keep this package limited to values with actual source/control consumers. Source bytes belong to search-revision-store. Durable current rows and compare-and-set operations belong to search-control-redb. Do not restore receipts, admission provenance, history, replay, batch codecs, migrations, or forwarding exports.

SourceId and SourceNamespaceId are random persisted values. Paths are locators; BLAKE3 digests identify content; NativeFileKey identifies the opened filesystem object. Keep these identities distinct.
