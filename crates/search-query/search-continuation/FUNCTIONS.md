# Same-session query windows

The unused continuation Rust package, stores, manifest and exclusive fixtures are removed.
The current owned child handles one query; this is policy for a later connected session consumer.

A session may retain a bounded final-order evidence vector, its pinned view, cursor, fixed expiry
and emitted-item indexes. Public (window_id, item_index) values locate only already emitted items
in that same session. One-shot output has no reusable locator.
Expiry/disconnect releases state; access does not renew expiry or reuse IDs.

Reserve finite item, metadata and optional excerpt memory before retrieval. Include temporary
candidate values and encoded output in the actual bounds. Never list/hash the live source catalogue
to authorize a pinned view. Read only a finite next prefix, preserving final rank.
Handled cards/gaps advance progress; untouched candidates do not become gaps.
An oversized item has an explicit bounded outcome rather than an endlessly empty page.

## Operation ownership

One session owner services the active operation and cancellation. Own the needed pending state;
hold no mutable map borrow or mutex across object reads, encoding or output.
Prepare the cursor/emitted-index update before output and apply it only after complete local flush.
Partial output closes the session and commits no progress. Healthy pre-output failure leaves the
old cursor unchanged; work already spent stays charged. Local flush is not peer acknowledgement.

Expansion stays inside a bounded neighborhood of an emitted item's retained representation/range.
No durable resume/replan, bearer tokens, grants, receipt history or second window service.
