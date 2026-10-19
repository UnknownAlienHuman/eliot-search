# Query-window lifetime and limits

Ordinary interactive windows are process-local and finite. The manager enforces configured ceilings before
retaining work:

```text
max live windows
max refs per window
max total retained refs / metadata bytes
max optional cached excerpt bytes
max page items / source-read bytes / encoded output bytes
absolute TTL
```

The query request's remaining counters apply while resolving/emitting a page. A window cannot renew its
absolute expiry by continuing or expanding it. Finishing a page does not extend the deadline.

Expiry, disconnect, cancellation, daemon shutdown or restart removes the window and drops its
`PublishedViewGuard`. No durable continuation, retention lease, terminal record or compaction history is
required. A restricted-client profile may choose shorter limits and bind windows to its concrete session;
it cannot widen the pinned view.

Reject new windows before allocating their retained vector when aggregate capacity is unavailable. Do not
collect an unbounded candidate set and truncate it after retention.
