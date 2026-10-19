# Historical W7 continuation hardening

The previous W7 direction attempted to turn interactive paging into a durable security/replay capability with
token digests, owner checkpoints, issued fingerprints, purge generations and cleanup receipts. That system
is not needed by the current local product and is not the daemon's serving implementation.

Use one process-local `QueryWindow` pinned to the actual `PublishedView`. A real restricted-client adapter
binds the window to its concrete connection/session and cancels it directly. Owning-account local use needs
no bearer authorization protocol.

Preserve only finite limits, exact selected-page evidence reads and the exclusive commit-after-complete-output
mechanic. Restart may invalidate every window. Remove durable replan, checkpoint, grant, security-generation,
terminal-history and cleanup-effect APIs with their fixtures.

See [README.md](README.md) and [FUNCTIONS.md](FUNCTIONS.md).
