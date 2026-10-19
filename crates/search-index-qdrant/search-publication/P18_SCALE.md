# Physical-generation replacement

The old P18 scale programme is not a current product subsystem. Do not implement `SCALE_PLANNED → R0 →
catch-up → R1 → final barrier → route switch → rollback` as another durable state machine.

A schema, lexical profile or vector-layout incompatibility is an ordinary publication whose target is a new
physical collection generation:

```text
one consistent current-source/preparation snapshot
→ bounded publication pages into the new generation
→ exact Qdrant readback
→ CurrentPublishedView route/generation commit
→ atomic current-view swap
→ old-view guard drain
→ delete the old collection
```

The old collection remains the committed query route until the new view commits. A failed or incomplete
candidate therefore cannot hide current points. Qdrant aliases may be an implementation detail, never the
visibility commit.

Ingestion may continue changing current source heads while a large replacement is built. Before commit,
compare the source/prepared snapshot named by the operation artifact with current state. Baseline may
serialize the replacement, replan it, or apply one bounded ordered delta before final readback. Do not add a
durable change-log/catch-up/barrier platform until measured scale requires one.

After possible Qdrant dispatch, resume the same `PendingPublication`. Failure before view commit leaves the
old view serving. Failure after view commit rebuilds the current pointer and finishes cleanup. A later
rollback, if ever needed, is another forward publication to a verified generation; it is not epoch rewind.

No candidate-quarantine ledger, R0/R1 receipts, final-barrier token, skipped sequence, `SCALE_BLOCKED`,
`QUARANTINED` or separate scale recovery owner is required. The same artifact, cursor, current view, guards
and cleanup rules in [FUNCTIONS.md](FUNCTIONS.md) own both small updates and full generation replacement.
