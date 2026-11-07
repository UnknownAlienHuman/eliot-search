# Shared values for the current search path

SourceId and SourceNamespaceId retain distinct 16-byte identities and canonical lowercase UUID parsing.
Blake3Digest32 carries the exact 32 digest bytes used by immutable object references.
MAX_FRAME_BYTES is the shared complete query-frame ceiling, including its four-byte prefix.

Actual object hashing/verification belongs to the object owner. Protocol DTOs and frame validation
belong to search-provider-protocol; publication rows and state belong to CONTROL.
See [FUNCTIONS.md](FUNCTIONS.md).
