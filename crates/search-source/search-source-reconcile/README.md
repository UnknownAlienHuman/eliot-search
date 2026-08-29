# search-source-reconcile

**C05 — Change reconciliation.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Turn watcher/USN hints and bounded inventories into truthful currentness, shadows and reconciliation work.

## Owns

- watcher hint ingestion
- cursor continuity and gap state
- startup/resume/periodic reconciliation plans
- inventory diffs and source-head observations
- observation freshness classification

## Must not own

- treating watchers as complete source truth
- reading file bytes directly
- publishing index epochs
- claiming current workspace across a gap
- depending on the concrete redb adapter; durable state is reached through a vendor-neutral port

- **Delivery wave:** W5 / P09
- **Soft source-line target:** 7,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)
