# Source family

**Organizational capability family — not a Cargo package.**

## Child packages

- [`search-source-registry/`](search-source-registry/) — C03: Own admitted roots, source memberships, reference portfolios and coherent SourceView/WorkspaceViewRevision resolution.
- [`search-source-identity/`](search-source-identity/) — C04: Derive stable source identity, retain path history and enforce single-writer namespace ownership and cutover.
- [`search-source-reconcile/`](search-source-reconcile/) — C05: Turn watcher/USN hints and bounded inventories into truthful currentness, shadows and reconciliation work.
- [`search-safe-reader/`](search-safe-reader/) — C06: Acquire exact source bytes without executing content, escaping admitted roots or mislabeling unstable files.
- [`search-revision-store/`](search-revision-store/) — C07: Admit, retain and reopen immutable source revisions under complete residency identities.

## Family invariants

- Paths are locators, not identity.
- Watchers are hints; inventory reconciliation proves observation continuity.
- Only exact bytes or immutable admitted revisions are source truth.
- SourceIdentity carries no membership or access policy.

Each writer agent owns exactly one child package and follows that package's `AGENTS.md`.
