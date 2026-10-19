# Historical W7 handle hardening

The previous W7 direction attempted to turn result handles into durable bearer/security records with grant,
checkpoint, invalidation, purge and retention-lease lifecycles. That framework is not connected to public
serving and is not needed by the current local product.

Use one process-local `QueryWindow` pinned to the actual `PublishedView`. A normal card contains its verified
source/revision/location/excerpt directly. Optional expansion uses `(window_id, item_index)` and can only
read a bounded neighborhood around that exact manifest item.

Owning-account local use needs no signed token or durable handle authority. A selected restricted-client
adapter binds the window to its real session and removes it on revocation.

Remove handle grants, permits, token digests, invalidation histories, retention leases and purge barriers
with their callers. Preserve only exact retained-range verification, finite limits and exclusive output
preparation described in [FUNCTIONS.md](FUNCTIONS.md).
