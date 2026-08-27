# search-control-redb

**Cell C02 — Control Journal.**

Bounded durable control state. The journal exists because cross-process lifecycle and crash recovery
need durable technical state, not because Search needs a second database.

- **Owns:** installation and data-root owner epoch; source roots and observed heads; membership and
  policy binding identifiers; manifest references; publication intents and receipts; committed visible
  epoch; collection route; watcher cursors; shadow, deny and purge fences; bounded job checkpoints.
- **Must not own:** source bodies or extracted corpus text; postings or term statistics; vectors;
  ranked candidate sets used as a query store; agent query history as an index.

A lost or incompatible journal causes a new collection generation and a rebuild. Authoritative
currentness is never reverse-engineered from an orphaned index.
