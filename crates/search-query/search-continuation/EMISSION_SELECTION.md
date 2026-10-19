# Query-window page selection

Selection is a direct slice of one final ordered `EvidenceRef[]` under the window's pinned view:

```text
start = next_index
end = min(start + requested_page_size, refs.len())
selected = refs[start..end]
```

Reserve item/source-read/output budget before retained-source I/O. Resolve and verify only `selected`.
Preserve its order through card formatting. Do not rescan prior items, recompute ranking, replan retrieval or
build an issued-fingerprint set.

The first page stages a new window; later pages hold an exclusive borrow of the existing window. The cursor
is inserted/advanced only after one complete output frame succeeds. Failed/partial output advances zero and
closes or drops the session/window according to the protocol fail-stop rule.

An item becomes eligible for `WindowItemRef` expansion only after its containing page was emitted
successfully. This is ordinary local state, not a durable issuance receipt.
