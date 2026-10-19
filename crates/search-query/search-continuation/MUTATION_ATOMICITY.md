# Query-window output atomicity

The useful atomic boundary is one in-process exclusive preparation, not a durable mutation protocol.

```text
prepare page under &mut QueryWindowManager
→ keep insertion/cursor change uncommitted
→ resolve evidence and format bounded frame
→ check original absolute deadline
→ write/flush complete frame
→ commit insert/advance/exhaustion
```

Dropping the preparation or returning an error before complete output leaves a new window absent or an
existing cursor unchanged. A partial transport write is not retryable application state: fail the
connection/session and drop or invalidate the window so duplicate partial frames are never treated as an
acknowledged page.

The exclusive borrow prevents concurrent continue/expand/invalidate operations from racing the prepared
page. No cloneable emission permit, post-output revalidation, durable receipt or external cleanup effect is
required.
