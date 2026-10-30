# Working on search-contracts

Read the relevant section of [FUNCTIONS.md](FUNCTIONS.md), then the actual producer and consumer. Do not preload
all query/protocol types or construct every operation before its handler exists.

Use the [representation rule](FUNCTIONS.md#one-representation-per-meaning) and root implementation choices.
Shared immutable data is not owner-private state, a vendor-neutral SDK copy or a catalogue of future nouns.
Keep distinct types for real invariants; do not add field-for-field DTO/plan/receipt conversion chains.

Change the selected handler and its real consumers, then remove displaced aliases, codecs, exports and exclusive
fixtures. There is no pre-release wire-compatibility facade or V3 rescue family. Keep content integrity and
actual external/persisted validation; no hashes or flags replace it.

After code run minimal locked check/Clippy for that production closure. Later behavior checks use the receiving
operation; old recipe counts and generated contract completeness are not gates.
