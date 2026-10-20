# Source reconciliation settings disposition

`[reconcile]` is not registered by the current daemon. The interval/budget/root/slice catalogue is a historical
proposal.

The target uses one bounded dirty queue fed by inventory scans and watcher hints. Overflow or uncertain
continuity schedules an authoritative affected-root scan; incomplete scans never retire unseen files. Expose
scan/watch cadence or slice bounds only after the connected owner consumes them.

Selected source roots are direct typed paths. A setting cannot make a gap current, issue a reconciliation
receipt or create a second source registry.
