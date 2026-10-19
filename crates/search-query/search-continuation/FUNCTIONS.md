# search-continuation — current source and replacement map

## Shared package

| Current API/area | Actual behavior |
|---|---|
| `ContinuationStore::create*` | Inserts an in-memory candidate window or durable-replan descriptor from caller-supplied token/binding records. It owns no real view guard. |
| `resolve` / `resume` | Resolves token digest, starts from the beginning, skips issued fingerprints and returns caller-executed effects. It performs no retrieval or source I/O. |
| `bind_durable_emission` | Binds a caller selection; it creates no durable query result or replan. |
| `revalidate_emission` / `commit_emission` | Separate pre/post-output phases over cloneable permits and issued-set history. |
| lifetime/quota/security modules | Apply caller-supplied generations/limits, retain terminal metadata and return external cleanup effects. |
| source inventory | Production top-level files exceed 79 KiB before nested security code; tests and fixtures are substantially larger. |

The package is not used by the public paging commands.

## Public daemon path

`cmd_search_page` and `cmd_continue` use a separate `ContinuationCatalog` and then mint another handle for
every emitted match.

Current daemon continuation mechanics:

```text
ContinuationRecord {
  namespace/session tag
  source_fence_digest
  Vec<StoredMatch>
  next_index
  coverage
  expiry
}
```

`source_fence::digest` calls `list_sources()` and hashes every source ID, revision, content/path digest,
length, active flag and sequence. Continuation creation and every later page recompute that whole-catalog
fence.

The useful code is narrow:

- one retained final-order vector;
- direct `next_index` slicing;
- an exclusive `PreparedPage` borrow;
- insertion/cursor advancement only after complete output callback success.

Keep those mechanics, not the authority model.

## Replacement owner

One daemon/query-owned manager stores:

```text
BTreeMap<WindowId, QueryWindow>

QueryWindow {
  PublishedViewGuard
  EvidenceRef[]
  next_index
  expiry
  finite metadata/read/output limits
  optional restricted-session identity
}
```

A monotonic/random process-local ID may be used as a locator. It is not a bearer authorization system for
the owning-account local pipe. The manager has finite aggregate caps and ordinary expiry cleanup.

### Create/first page

```text
prepare_first_page(final_refs, view_guard, limits)
→ PreparedWindowPage
→ resolve selected evidence
→ emit complete frame
→ insert window with cursor advanced
```

Dropping or failed output inserts nothing. If every item fits and no expansion locator is needed, no window
must be retained.

### Continue

```text
prepare_next_page(window_id, page_size, request_context)
→ exclusive PreparedWindowPage
→ resolve only selected refs
→ emit complete frame
→ advance cursor or mark exhausted
```

No source-catalog digest, replan descriptor, issued-fingerprint scan or later-view comparison occurs. The
window's pinned view is the fence.

### Expand

```text
expand(WindowItemRef, bounded neighborhood request, context)
→ exact manifest item under same view
→ retained object/range verification
→ bounded excerpt
```

The original item index must have been exposed by a successfully emitted page. Expansion cannot request an
arbitrary revision range. It does not mint another handle.

### Expire/drop

Remove the map entry and drop its view guard/cached content. No external acknowledgement, pin-release receipt,
terminal history or durable cleanup is required. If a restricted session is revoked, its owner removes its
windows and cancels in-flight work directly.

## Delete after caller cutover

- shared `ContinuationStore`, token/security/durable-replan/issued-set/terminal-cleanup model;
- daemon source-fence/session-tag/token catalogue;
- `LiveExpansionBarrier` and caller-created clean/checkpoint values;
- separate continuation contract lifecycle records;
- durable checkpoint, epoch-pin, grant and purge cleanup effects;
- all fixtures preserving those APIs.

Delete the crate after useful prepared-page code is moved. Do not leave it as a forwarding facade.

## Verification

- first-page output failure leaves no window;
- later-page output failure advances zero and closes/drops safely;
- page slicing is O(page), not O(prefix) or O(corpus);
- only selected-page evidence is read;
- exact old view remains alive while the window is live;
- unrelated new source/view state does not invalidate or rescan the window;
- expiry/restart/disconnect release the guard;
- expansion is restricted to successfully issued item indexes and bounded context;
- finite aggregate metadata/excerpt limits are enforced before retention.
