# Immutable-object storage settings disposition

`[revision_store]` is not registered by the current daemon. The old quota/temp/fsync/reopen packet is not a
supported settings API.

The target object owner uses one current-format storage profile and enforces its input/object/staging limits.
Final publication/readback and the selected durability guarantee are implementation invariants. The root owns
its storage profile; a profile change requires a new root/export/rebuild, not live reconfiguration.

Add an actual quota only with bounded GC/backpressure behavior. Do not create a generic storage section digest,
safe-plan receipt or second object store. Storage protection is described in `secrets.md`.
