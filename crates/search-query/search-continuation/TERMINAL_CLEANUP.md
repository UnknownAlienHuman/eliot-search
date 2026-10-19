# Query-window terminal cleanup

Terminal cleanup is ordinary ownership release:

```text
remove QueryWindow
→ drop optional cached excerpts and evidence refs
→ decrement manager counters
→ drop PublishedViewGuard
```

Completion may remove the cursor while retaining the window until expiry if successfully issued
`WindowItemRef` values still need expansion. Otherwise an exhausted window can be removed immediately.
Expiry, disconnect, cancellation, restricted-session revocation, daemon shutdown and restart remove the
whole entry.

There is no durable checkpoint to delete, epoch pin to acknowledge, terminal history to compact or cleanup
effect to return to a caller. The view guard's final drop is the only lifetime signal needed by publication
cleanup. If removal occurs during a prepared output, the exclusive borrow/output fail-stop rule resolves the
operation before the entry can disappear.
