# Handle and source-expansion settings

Follow the [query implementation](../../docs/implementation/README.md#4-simplify-and-connect-the-query).
The old `[handles]` table is a proposal in the example/reference; the current
[daemon registry](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs) does not register
that section. Its old TTL and per-binding keys are not an implemented file-to-runtime API.

## Current runtime values

These are Rust fields/constants, not supported TOML keys.

| Existing owner | Current fields or constants |
|---|---|
| [HandlePolicy](../../crates/search-query/search-handles/src/lib.rs) | `max_ephemeral_records` and `max_durable_records`: 16,384 each, counting active records across that store; `max_expire_batch` and `max_invalidate_batch`: 1,024 each. |
| `HandlePolicy.max_expansion_bytes` | Baseline 8 MiB; checked for nonzero during policy validation, but not consumed by `revalidate` or `expand`. It does not currently enforce that ceiling. |
| [Daemon DIRECT constants](../../bins/eliot-searchd/src/result_handles/kernel/spec.rs) | `MAX_RESULT_HANDLES`: 50,000; `MAX_HANDLE_EXPANSION_BYTES`: 24 KiB; `RESULT_HANDLE_TTL`: 15 minutes. This is a separate catalog used by public DIRECT queries. |

Shared `HandleStore` accepts policy at construction and exposes mint/resolve/expire/invalidate; it has
no `apply_live_limits` or section reload API. Expiration timestamps arrive with mint material, not a
policy TTL field. Both shared handle classes are in memory. See
[actual functions](../../crates/search-query/search-handles/FUNCTIONS.md).

## Target

Return exact source/revision/range references for trusted-local results. Any selected opaque locator
uses the [same temporary window](../../crates/search-query/search-continuation/README.md#one-temporary-owner)
as paging. Replace both catalogs with that owner and change their real callers; preserve no separate
durable-handle configuration platform.

Expose only limits the surviving owner enforces: retained item/byte capacity, expiry and source-read
byte/time ceilings. Apply the expansion limit before the actual read. Delete expired state rather than
retaining terminal history, and keep issued locators valid until their stated expiry unless their
session ends or a real restriction invalidates them. Selected restricted clients retain actual scope,
revocation and disclosure rules.

Construct from validated settings. Implement a runtime limit change only with its real consumer;
no section digest, generic reload receipt or compulsory live-update API is required.
