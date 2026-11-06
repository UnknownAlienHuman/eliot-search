# Working on current source values

Keep values limited to actual source/control/unitizer/binary consumers: CurrentSourceRecord, ObjectRef kinds, output-affecting PreparationSettings and shared bounds. Bytes belong to search-revision-store, artifact encoding to unitizer, durable revisions and CAS to CONTROL. Fail-only limits are separate from settings identity. Do not restore receipts, provenance, history, replay, batch codecs, migrations or forwarding exports.

SourceId and SourceNamespaceId are random persisted values. Paths are locators; BLAKE3 digests identify content; NativeFileKey identifies the opened filesystem object. Keep these identities distinct.
