# Working on search-retention

This package is a legacy removal target. Read the root
[architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and local
[function map](FUNCTIONS.md).

Allowed work:

- identify and switch the last real caller to current/pending/pinned view lifetime;
- move a useful exact effect helper to its actual source/publication/bridge owner;
- delete obsolete lease, sweep, purge, restore, migration, fixture and feature code;
- keep unsupported old roots untouched.

Do not add:

- a durable lease catalogue or replay journal;
- legal-hold/security-purge planes, tombstone protocols or secure-erasure claims;
- a generic object graph / CAS mark-and-sweep service;
- live restore, root cutover, re-encryption or compatibility migration;
- new proofs, receipts, canonical digests, ports, packages or line quotas.

Ordinary local GC is described by the published view and exact references. If deletion is uncertain, retain
bytes and retry later. The safe failure is a leak, not a larger lifecycle operating system.
