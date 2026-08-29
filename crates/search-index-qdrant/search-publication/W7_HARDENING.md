# W7 hardening — `search-publication`

This packet refines publication interaction with restrictive security, purge and restore. The existing
W3 `FUNCTIONS.md` remains authoritative for normal point publication/recovery.

## Invalidation-only publication

```text
commit_invalidation_only(command, control_port, snapshot_port, operation)
    -> Result<InvalidationPublicationReceipt, PublicationError>
```

Used when live security/currentness must advance without waiting for index cleanup/rebuild. One guarded
control transaction increments the relevant deny/shadow/purge generation, records exact affected scope
and publishes an immutable snapshot before acknowledgement.

No new point becomes current. Existing Qdrant bytes may remain physically present but are excluded before
retrieval/IDF/counts/traces by the live fence.

## Purge interaction

A purge/security fence is not a normal retired-point manifest and cannot be downgraded to ordinary
reclaim. Publication may emit exact security-purge index targets/receipts through the lifecycle port,
but `search-retention` owns purge plan/final receipt and `search-index-reclaimer` owns only ordinary
retirement.

Any staged/uncommitted intent overlapping a purge scope is fenced, recovered/compensated under its exact
intent and cannot later commit visibility. The allocated epoch remains consumed.

## Restore interaction

Restored Qdrant collection/snapshot never becomes the active route directly. It enters a candidate
quarantine generation. After lifecycle/source/access/profile/purge revalidation, publication performs
normal exact readback and one new guarded route/epoch commit. Old backup visible epoch is not reused or
trusted as current.

## Required tests

- invalidation-only commit live before acknowledgement;
- Qdrant unavailable during revocation still yields logical non-accessibility;
- purge racing every normal publication state cannot expose/stage-commit purged material;
- purge fence/targets/receipts distinct from ordinary retired manifest/reclaim;
- staged overlap is compensated/fenced and epoch never reused;
- restored old route cannot serve before new guarded publication;
- purge tombstone blocks restore/republication resurrection;
- restore route commit uses current owner/access/profile/source guards;
- failure after control commit before snapshot publication remains fail-closed/recoverable.
