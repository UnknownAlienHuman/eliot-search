# search-handles — remove the second result store

A search result does not need a separately minted source authority. The card can carry its exact source,
revision, representation, location and verified excerpt. Optional expansion is an item locator inside the
same short-lived `QueryWindow`.

Read the [query cutover](../README.md), [query-window map](../search-continuation/FUNCTIONS.md) and local
[source map](FUNCTIONS.md).

## Verified current source

The shared package is about 35 KiB of production Rust and is not the public daemon's handle store. It defines
another token/record/invalidation lifecycle with caller-created validation/permit structures.

The daemon separately has roughly 19 KiB of production handle catalogue/expansion code. For every emitted
match it:

1. enumerates every source into a map;
2. recomputes a SHA-256 fence over the entire source catalogue;
3. generates one random 32-byte-derived token;
4. stores namespace/session/source/revision/digest/length/expiry again.

Expansion rehashes the whole source catalogue, enumerates sources again and accepts any caller byte range
inside the entire retained revision up to a maximum size. The handle is therefore both redundant and broader
than the evidence item which caused it to be issued.

Do not migrate the daemon to the shared package. Delete both models.

## Replacement

A normal `EvidenceCard` contains:

```text
source and exact retained revision/representation
logical/byte/line location
short verified excerpt
rank/contribution metadata
optional WindowItemRef { window_id, item_index }
```

`WindowItemRef` is a process-local locator into the exact final-order item already emitted from one
`QueryWindow`. It resolves under the same `PublishedViewGuard`, expiry and optional restricted session.

Expansion may request a finite context policy such as additional lines/bytes around that item's manifest
range. The owner clamps the request to the configured neighborhood and immutable object boundaries. It may
not name arbitrary absolute revision offsets, another item or another view.

Useful mechanics to move:

- exact retained revision/range read;
- actual returned byte-length/range verification;
- finite expansion byte ceiling;
- keeping an exclusive window/manager borrow through output;
- original absolute expiry/deadline check before write;
- no state change after partial output.

The immutable reader, not echoed digests, proves the object. The pinned manifest supplies the authorized
range. No source-catalog fence or current source enumeration is needed.

## Identity and security

For the owning-account local pipe, `(window_id, item_index)` is not a cryptographic bearer token. The caller
already owns the local session and the window exists only in that process. A selected restricted-client
adapter binds the window to its real connection/session and disclosure rules; revocation cancels and removes
it.

Do not add handle token signatures, entropy providers, durable retention leases, grant checkpoints,
invalidation generations or purge histories to the baseline.

## Remove after cutover

Delete:

```text
crates/search-query/search-handles/
bins/eliot-searchd/src/result_handles/
SearchSourceHandle and handle lifecycle contracts
source_fence hashing for result expansion
per-item random token generation and record maps
handle validation/permit/invalidation/security fixtures
```

Change current protocol/result consumers together; do not keep forwarding handle wrappers or a V3 family.

## Completion

A result page emits the correct verified excerpt without per-item token minting or whole-catalog scans.
Bounded expansion can only enlarge the selected item's context under the same view/window and fails after
expiry/restart/session revocation. No separate handle database remains reachable.
