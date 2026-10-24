# View lifetime: pin a real snapshot, not a route label

At `c8e38b6`, PinRegistry exposes arbitrary route/epoch acquisition, owner release/renewal, sampled watermarks
and a second continuation catalogue. Those records do not prevent new old pins after a cleanup decision.
Remove the public registry with its callers.

The surviving manager is private to publication/query: one strong current state and weak references to states
still held by requests/windows. The actual state includes a redb read snapshot of the published source and
evidence tables, not only a sequence and a pointer to mutable latest metadata.

The [publication recipe](../search-publication/FUNCTIONS.md) defines commit, snapshot capture, installation,
cleanup eligibility and restart. The [control map](../../search-control-redb/FUNCTIONS.md) defines lookup.
Do not copy those algorithms here or introduce a new snapshot service.

Acquire only current under a short mutex. A live window inherits its request's guard; callers cannot reopen
old views by ID. Expiry/disconnect releases it. Last drop may wake cleanup and never performs I/O. Old redb
pages remain retained while readers exist; finite query/window lifetimes and resource backpressure must be
measured, not inferred from a small Arc count.

No view-manager lock across redb/object/Qdrant work. There is no live local-object GC gate in the selected
baseline: local collection owns the stopped root exclusively. Qdrant cleanup remains live and waits for all
relevant view sequences/generation users. Keep only a small helper in the actual owner and delete this crate.
