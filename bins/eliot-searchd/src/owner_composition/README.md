# Legacy daemon root composition — cutover map

This directory mixes root ownership with initialization, source admission/registration, registry replay,
key selection, migration and command routing. It is not the target application boundary.

[Daemon composition](../../FUNCTIONS.md) owns the connected entrypoint.
[Root](../../../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) owns current-format classification,
initialization, actual exclusion and shared platform finalization. Read those recipes, not another root algorithm
here. The [prior inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/bins/eliot-searchd/src/owner_composition/README.md)
preserves historical size and source-family observations; recheck actual callers at the code cut.

## Receiving owners

| Existing area | Move useful behavior to |
|---|---|
| NativeRootRequest / command-shaped parsing | Single daemon invocation/settings owner |
| Path, locality, exclusion, manifest initialization | Current root/platform owner |
| Registry restore and current source state | Typed redb/source owner |
| Source admission/read/sync | Source acquisition and reconciliation |
| Revision key selection | Accepted #598 object/key profile only |
| One-shot/service wrappers | Current dispatcher and actual handlers |

Keep the actual root guard across store lifetime, not an owner receipt per request. Initialize manifest-last;
only recognized empty incomplete state may be explicitly discarded and retried. No resume/replay or repair on
ordinary open. Unknown/user data remains untouched. The root recipe alone defines platform requirements.

Move direct consumers and remove displaced command wrappers, initialization journals, owner slots/epochs,
succession, dual legacy/sealed locks, operation digests and migration/receipt machinery together. Do not connect
all old applications under another manager. Preserve useful verified-open/identity checks and user data.

Unpublished #598 work remains protected. Consume only its accepted public handoff; plain Foundation does not
wait for it. After Rust changes run minimal pinned check/Clippy for actual root/daemon/store consumers.
