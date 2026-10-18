# Temporary query-window settings

Follow the [one-window implementation](../../crates/search-query/search-continuation/README.md#one-temporary-owner).
The old `[continuations]` table is a proposal in the example/reference; the current
[daemon registry](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs) does not register
that section. Setting its old TTL or per-binding fields does not configure public paging.

## Current runtime values

These are separate Rust implementations, not one supported settings API.

| Existing owner | Current fields or constants |
|---|---|
| [ContinuationLimits](../../crates/search-query/search-continuation/src/lib.rs) | Baseline `max_records`: 1,024 including terminal metadata; `max_ephemeral_per_binding`: 128; `max_durable_per_binding`: 32. |
| Window/page limits in `ContinuationLimits` | `max_candidate_window` / `max_issued_candidates`: shared list/set ceilings; `max_expansion_items` / `max_lifecycle_batch`: 256; `max_ttl_millis`: 900,000. |
| [Daemon DIRECT constants](../../bins/eliot-searchd/src/continuation/kernel/spec.rs) | `MAX_CONTINUATIONS`: 16; `MAX_RETAINED_MATCHES`: 25,000 total; `MAX_PAGE_SIZE`: 1,000, default 100; `MAX_GAP_DETAILS`: 256; `CONTINUATION_TTL`: 15 minutes. |

Shared `ContinuationStore` accepts limits at construction and has `apply_live_limits`, expiry and
compaction APIs. These change in-memory state and return effects; they do not apply TOML settings,
perform pin cleanup or persist durable replan descriptors themselves. Public paging uses the separate
daemon catalog. See [actual functions](../../crates/search-query/search-continuation/FUNCTIONS.md).

## Target

Keep one owner for the ordered window, next index, real scope/view resources and absolute expiry.
Use its concrete item/byte/window limits and a bounded page size; connect those accepted settings to
the public handler. Reuse the existing daemon cursor and commit-after-delivery mechanics, then remove
the duplicate engine and unconsumed settings.

New defaults affect new requests/windows. Admitted work keeps its own deadline and fixed expiry.
A supported restrictive change can expire affected state and release its resources; a setting edit
alone does neither. Last-page delivery cannot discard items still backing live expansion locators.
Restart may expire temporary state; ordinary queries need no durable checkpoint/issued-ID history.

Keep actual selected-scope restrictions and source validation. No whole-catalog fingerprint,
section digest or generic reload-receipt system is needed for window limits.
