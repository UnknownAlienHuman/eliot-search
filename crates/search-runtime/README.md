# Runtime family

Organizational directory, not a Cargo package.

## Active responsibility

- [`search-runtime-owner/`](search-runtime-owner/) owns one process/data-root owner epoch and its real
  acquire, drain and release effects.
- [`search-os-secrets/`](../search-runtime/search-os-secrets/) owns optional OS-bound secret references.
- The daemon composes runtime owners with source, control, publication and query behavior.

[`search-retention/`](search-retention/) is a legacy W7 lifecycle implementation. Its generic leases,
mark/sweep, purge and restore coordinators are not the baseline product. Useful ordinary lifetime behavior
moves to current/pending/pinned published views and their owning source/publication stores; the package and
`wave7-lifecycle` are removed after callers/tests switch.

Runtime code does not decide retrieval meaning, source evidence or query access. Do not add a family-level
Cargo package or another lifecycle framework.
