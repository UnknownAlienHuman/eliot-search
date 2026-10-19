# Historical security-mutation barrier

This file previously specified a durable access-policy/restriction transaction, live deny publication,
phase checkpoints and dependent receipts around query execution. That system is not part of the current
local product.

The owning-account baseline selects a non-widening `ResolvedScope` from one pinned `PublishedView` and uses
that same population for retrieval, IDF, exact scan and evidence lookup. A source removed from a later view
cannot enter a new request; an already pinned local request may finish coherently under its old immutable
view.

A selected restricted-client adapter owns its actual connection/session policy:

```text
allowed source/view subset
request/output limits
expiry/revocation
window ownership
cancellation token
```

Revocation closes/cancels that session and expires its windows. It does not require a generic durable
security mutation, influence graph, purge receipt, grant-generation barrier or checkpoint permit inside
every query package.

Publication reflects later selected-source changes through the ordinary pending operation and view switch.
Physical Qdrant bytes are not access authority. Do not implement invalidation-only publication or query
compensation to preserve the old barrier.

Remove `SecurityMutationBarrier`, its injected effects/receipts and exclusive fixtures with their callers.
Keep only direct `ResolvedScope` intersection and concrete optional restricted-session enforcement described
in [README.md](README.md) and [FUNCTIONS.md](FUNCTIONS.md).
