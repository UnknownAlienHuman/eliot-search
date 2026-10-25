# search-handles — inspected source and removal map

## Evidence boundary

At the established audit revision, the shared package contained about 35 KiB of production Rust and the daemon
had another roughly 19 KiB result-handle catalogue. The public daemon did not use the shared package. These size,
topology and caller findings are historical evidence, not current branch truth.

Before a code cut, repeat the exact current search across manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, features, targets, tests and workflows. Record all surviving consumers and move them before
removing APIs, modules or the package.

## Inspected behavior

The shared package stored token-digest records and copied source/revision/range metadata in another in-memory
authority map, accepted caller-shaped validation/permit structures and maintained an invalidation lifecycle.

The inspected daemon path listed and hashed the whole source catalogue, minted a random token per emitted item,
stored source/revision/digest/length/expiry again, then repeated catalogue work during expansion. Expansion could
accept arbitrary byte ranges inside the whole revision. Staged insertion only after complete output and keeping an
exclusive borrow through output were useful mechanics; the second authority and broad range were not.

## Replacement

No public handle mint exists. The query-window owner may expose:

```text
WindowItemRef { window_id, item_index }
```

`prepare_expand` resolves the exact successfully emitted item under an exclusive live-window borrow, checks
absolute expiry and any concrete restricted-session binding, obtains its pinned manifest entry, derives a finite
relative neighborhood, reserves read/output budget and verifies the retained object/range. The request cannot
supply unconstrained absolute offsets.

Complete output returns the frame; partial output fails the session/window without minting or acknowledging new
state. The pinned manifest supplies source authority and the immutable reader verifies bytes.

## Delete after caller cutover

Remove both handle stores, token entropy/hashing, copied records, source-catalogue fences/scans, grants/permits,
invalidation generations, durable leases, arbitrary revision-range expansion and handle-specific crypto/provider
fixtures. Move exact range reading and exclusive prepared-output behavior into the query-window/evidence owner and
leave no facade.

## Verification after code

Run minimal Rust 1.98 check/Clippy for changed production consumers. Verify no handle allocation for ordinary
cards, no whole-catalogue enumeration for page/expansion, only emitted items can expand, neighborhoods stay
bounded, wrong/missing bytes fail real verification, and expiry/restart/disconnect/revocation remove availability.
