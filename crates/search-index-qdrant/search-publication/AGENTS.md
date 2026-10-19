# Working on search-publication

Read the root [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).
Change planner, typed redb state, Qdrant bridge, current-view owner and query lookup together.

Implement one operation:

```text
ordered prepared pages
→ immutable operation/manifest artifact
→ one PendingPublication cursor
→ actual Qdrant mutation and exact readback
→ CurrentPublishedView commit
→ current-view swap
→ old-view drain and cleanup
```

Keep the useful view-sequence interval: new points begin at the target sequence and old points end at it.
Only the committed current view selects which sequence queries use. Do not recreate `PublicationPhase`,
caller-generated receipts, snapshot acknowledgements or a completed-operation history around this rule.

Before pending persistence, local work is disposable. After any Qdrant request may have committed, preserve
the same operation/page identity, inspect actual state and finish that operation. Do not infer rollback,
allocate a replacement sequence, compensate from receipt lists, abandon through a membership fence or
introduce invalidation-only publication.

Full physical-generation replacement uses the same publisher and cursor. Restricted-client scope belongs to
query/access composition, not a security/purge publication state machine. Cleanup is the terminal part of
this operation after old view guards drain; it has no separate proof/permit/journal platform.

Delete old phase/restore/abort APIs and their package/process/source-shape fixtures as actual consumers move.
Do not preserve them through a facade. Verify the connected daemon around real object/redb/Qdrant/view
boundaries. Run minimal Rust 1.98 compiler/Clippy for changed production owners after code; broad tests wait
until the product path exists.
