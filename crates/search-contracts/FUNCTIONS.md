# Shared values — actual consumers only

SourceId and SourceNamespaceId cross source/CONTROL/bridge boundaries. They retain their byte layout,
ordering, lowercase UUID parse and display semantics. Invalid UUID input returns a content-free error.
Blake3Digest32 is the 32-byte value inside actual ObjectRefs; wrapping stored or computed bytes performs no hash.
The object owner independently computes and verifies maintained BLAKE3 over retained bytes.
MAX_FRAME_BYTES remains 8388608, including the complete wire prefix.

## One representation per meaning

Only these values and their parsing support remain here. The protocol owns the actual current wire DTOs
and validation. CONTROL owns publication metadata. Runtime guards, settings and SDK types stay private
to their concrete owners.

The unused authority/recipe/plan/receipt/indexed/lifecycle/result/codec catalogues and exclusive fixtures
are removed. Add a shared value only when a real producer and consumer need its invariant.
No compatibility facade or test-only resurrection.
