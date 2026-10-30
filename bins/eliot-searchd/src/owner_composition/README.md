# Legacy daemon root composition — cutover map

This directory mixes root, configuration, source/registry, key-selection and migration responsibilities.
It is existing source, not the target application boundary. Trace actual callers before deleting it.

| Existing responsibility | Receiving owner |
|---|---|
| Invocation/settings | Daemon parser and typed settings |
| Root exclusion/init/open | [Root recipe](../../../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Source acquisition/current rows | Source reader and typed control state |
| Immutable bytes | Plain object store |
| DEK/pairing/security lifecycle | Remove with consumers under [local scope](../../../../docs/architecture/LOCAL_SCOPE.md); no replacement service |
| Serving/shutdown | [Daemon composition](../../FUNCTIONS.md) and actual client/protocol |

Keep one root guard across store lifetime. No owner epochs, journal/resume, generic command manager or #598 handoff
is needed. Preserve originals, unsupported old roots and unpublished work. Package/file removal is source work;
this map does not claim it has happened.
