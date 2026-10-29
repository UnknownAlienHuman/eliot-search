# Working on search-unitizer

Use the root [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md). The package owns useful ranges/occurrences;
it does not need a separate authority or replay system.

## Implementation

- Consume the existing materializer product and preserve its source revision, exact coordinates and
  actual loss semantics. Change producer/consumer types together where the current ingress obstructs this.
- Reuse [layout.rs](src/layout.rs) and the existing Markdown/JATS observations. Do not add a second
  parser, source reader or duplicate coordinate validator.
- Remove the unconditional materializer replay in structural construction. Private immutable products
  from the producer can pass directly to unitization; validate external/persisted input when accepting it.
- Build each unit and its necessary identity once. Encode typed records once when storing them;
  do not rebuild full value trees and byte buffers merely to enforce a size ceiling.
- Keep unit size/count/output bounds and the operation's existing cancellation/deadline. Use direct
  checks; source hashes, fixture bindings and nested logical-work ledgers are not a runtime scheduler.
- Keep one representation/UnitSet owner. Raw byte coverage and overlapping structural observations
  have distinct meanings, but do not require parallel serialized authority objects.

## Correct results

Current v3 accepts exact byte-identical UTF-8 only. Supporting another coordinate/transform policy
requires implementing its real mapping; materializer support alone does not establish unitizer support.
Preserve byte boundaries, deterministic ordinals, source anchors and clear unsupported input.
Do not turn overlapping structural ranges into an exhaustive denominator or claim compiler truth.

## Replacement and checks

Old profile IDs, v3/v4/v5 envelopes, DIRECT receipts and fixture bytes are not compatibility promises.
Remove replaced representations and exclusive consumers together. Do not invent dummy profile/fixture
digests or require a migration/export prerequisite. Preserve original files and sole retained copies.

Exercise changed boundary, coverage, repeated-content identity, parser-range and cancellation behavior
with the real preparation consumer. No package permission exception, line quota, mandatory full replay
at every handoff or historical qualification gate constrains the new implementation.
