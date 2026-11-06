# Daemon — root operations and one retained file

The daemon still owns the same native invocation capture, bootstrap settings, root dispatch, and factual output. The new retain-file operation extends native_root_command through the existing root-owned CLI child.

## retain-file path

1. Resolve --source-file against the captured startup directory while preserving native path bytes.
2. Open the existing root with open_plain and retain PlainRootGuard.
3. Capture a read-only current-source guard from the same root control.redb.
4. Call search-safe-reader acquire_file once for the selected path.
5. Move the acquired Vec to search-revision-store, publish one versioned source ObjectRef beneath guard.object_dir, and read the object back to verify exact bytes, length, and BLAKE3.
6. Commit one CurrentSourceRecord with namespace, random SourceId, NativeFileKey, ObjectRef, and row/registration CAS in the same control.redb.
7. Emit a compact factual JSON result only after the row commit. It reports IDs, revisions, file key, ObjectRef fields, and verified readback; it does not report search evidence.

The command has an 8 MiB bound and no execute, network, Git, inventory, watcher, materialization, Qdrant, or search path. Failures are content-free and never become successful empty results. Root initialization, inspection, savepoint safety, and incomplete discard are unchanged.

## Verification state

This source path is implemented on the source branch but is not yet runtime-qualified. The designated verifier owns the locked check/Clippy and native CLI capture, retained readback, mutation, stale-CAS, and refusal cases.
