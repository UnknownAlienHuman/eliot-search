# W7 hardening — `search-candidate-validator`

This packet refines restrictive-security races, purge and restore behavior for source-backed evidence.

## Checkpoint sequence

Validation performs live-fence checks:

1. before nomination admission;
2. before exact revision-readback request;
3. immediately after readback returns;
4. after anchor/digest/profile verification;
5. before source-handle request;
6. immediately before candidate/result emission.

Every check binds binding/grant, source namespace/owner generation, source/workspace view, access/live
deny, purge tombstone generation, overlay shadow, residency and retention requirements.

A restrictive change discards bytes and candidate state. If the scoring population was contaminated,
the validator returns a whole-leg replan/discard signal rather than filtering one candidate.

## `validate_lifecycle_fence`

```text
validate_lifecycle_fence(candidate, planned, live, checkpoint)
    -> LifecycleValidationDecision
```

Closed decisions: `PERMIT`, `DROP_GAP`, `CONTAMINATED_LEG`, `DENY_PURGED`, `DENY_REVOKED`,
`REPLAN_OWNER_OR_VIEW`, `FAIL_CLOSED_UNKNOWN`.

Unknown/missing lifecycle state is never permit. Decisions contain bounded reason/scope metadata only;
no inaccessible source identity/display data is exposed.

## Purge and restore

A purge tombstone dominates retained revision availability and stale Qdrant nomination. A source object
that still physically exists or was restored cannot be validated for the purged scope/owner generation.

Restored/rebuilt candidates remain ineligible until current source/owner/membership/access/residency,
profile/schema/publication and purge-tombstone receipts are accepted. Old backup epoch/route payload is
not evidence.

## Required tests

- revocation/purge at every checkpoint;
- exact readback completes then purge before emission: no excerpt/handle/candidate;
- contaminated population discards whole leg;
- unknown lifecycle state fails closed without source/name/count disclosure;
- physically retained/restored purged bytes cannot validate;
- restored old epoch/profile candidate rejected until new publication/admission;
- validation gap contains no evidence payload;
- fake live-state/readback ports prove no concrete lifecycle-store dependency.
