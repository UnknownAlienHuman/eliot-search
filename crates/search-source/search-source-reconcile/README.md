# search-source-reconcile

**C05 — Change reconciliation.**

**Status:** `SOURCE_PRESENT`; change-hint, inventory-diff and reconciliation planning source exists, while durable multi-root currentness and product integration remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #239 → #128.

Turn watcher hints and bounded inventories into truthful currentness, shadows and reconciliation work.

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
