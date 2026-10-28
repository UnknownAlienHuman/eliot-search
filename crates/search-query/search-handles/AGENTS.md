# Working on handles

Read [FUNCTIONS.md](FUNCTIONS.md) and the [window owner](../search-continuation/FUNCTIONS.md). Recheck current
protocol/result/daemon callers before removing both old handle catalogues; do not migrate one into the other.

Keep exact retained-range verification and bounded context around successfully emitted items. Use the window's
owned pending-delivery path, not a shared mutable borrow/lock across object reads or transport writes. A locator
cannot choose arbitrary revision offsets, renew expiry or switch view/session.

Remove token minting/hashing, catalogue scans/fences, grants, invalidation histories and exclusive fixtures with
callers. After code run minimal pinned check/Clippy for actual consumers. No forwarding handle platform remains.
