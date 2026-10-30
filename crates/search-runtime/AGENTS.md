# Working on runtime effects

Use the affected [owner](README.md), [local scope](../../docs/architecture/LOCAL_SCOPE.md) and actual callers.
One root guard, plain objects and an owned child are enough. Do not introduce a security-profile selector,
secret/lease catalogue, owner-epoch protocol or general process/filesystem framework.

Root exclusion is real locking, not lock-file existence. Local object GC is stopped-root work; Qdrant retirement
belongs to publication. Preserve real integrity, no-clobber behavior, cancellation and safe child cleanup.

Remove legacy crypto/pairing/transport code only with its current consumers, manifests/features and exclusive
fixtures. Preserve user keys/bytes and unpublished #598 work; neither requires a new product dependency.
Run the [minimal production checks](../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks), including
Windows code actually changed. Do not claim source deletion or runtime safety from a documentation edit.
