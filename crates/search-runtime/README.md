# Runtime family

**Organizational capability family — not a Cargo package.**

## Child packages

- [`search-runtime-owner/`](search-runtime-owner/) — C01: Guarantee that exactly one process incarnation owns one data root and expose a fenced lifecycle to the daemon.
- [`search-retention/`](search-retention/) — C28: Execute crash-safe mark-and-sweep, monotonic purge and restore quarantine across Search-owned projections and CAS.

## Family invariants

- Own lifecycle, retention and purge mechanics; never retrieval meaning.
- The daemon is the composition root; this directory itself is not a Cargo package.
- Security/legal purge dominates ordinary retention.

Each writer agent owns exactly one child package and follows that package's `AGENTS.md`.
