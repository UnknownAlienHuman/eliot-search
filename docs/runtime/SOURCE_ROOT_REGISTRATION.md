# Persistent observation-root registration

The primary `eliot-searchd` restores `control/source-roots.v1` after acquiring its existing data-root
OS lock, before publishing readiness. The same owner is used by one-shot commands and the persistent
DIRECT service. No additional daemon, Python process, database or implicit index is introduced.

Create the data directory first. Source directories must be separate from it: neither a parent nor a
child of the data root is admitted. Stop any existing owner before running the one-shot commands.

```sh
eliot-searchd --register-source-root DATA_ROOT SOURCE_DIRECTORY
eliot-searchd --source-roots DATA_ROOT
eliot-searchd --sync-source-roots DATA_ROOT
eliot-searchd --search-root DATA_ROOT QUERY
eliot-searchd --unregister-source-root DATA_ROOT SOURCE_DIRECTORY
```

Registration is retained across process restarts; `--sync-source-roots` uses that retained list without
requiring source arguments again. Registration itself does not ingest bytes. Unregistering stops this
explicit observation path; it neither revokes access to retained revisions nor purges them. Source IDs
are not derived from catalog positions. Existing explicit file/directory admission commands are unchanged.

## Failure semantics

The catalog has at most 32 non-overlapping absolute UTF-8 paths, each at most 512 bytes; the complete
file read is capped at 64 KiB. Exact path whitespace is preserved. Corrupt, truncated, duplicate or
unsupported catalog contents are rejected instead of being treated as an empty configuration. Symlink
and Windows reparse entries are rejected. Native handle-race qualification is still required.

Writes stage a bounded file, sync it, verify exact readback and replace the previous catalog. An uncertain
replacement blocks further operations on that instance until reopening. Recovery preserves the old
catalog when replacement did not reach the current name. A corrupt current catalog is never silently
replaced by an older backup. Unix parent-directory sync is implemented; Windows power-loss durability is
not claimed or accepted by this change.

All registered roots are checked before explicit synchronization begins. Missing, unsafe or unreadable
roots cause `SOURCE_ROOTS_UNAVAILABLE`, not retirement of their retained sources. Failure during a later
root's synchronization is explicitly partial: earlier source effects may already have committed. There
is no fabricated all-roots rollback receipt. Directory manifests and retained revisions still use the
existing DIRECT implementation.

A completed synchronization describes that explicit inventory operation, not a continuously current
workspace. Responses keep `current_workspace_proven=false` and `qdrant_available=false`. This does not
complete the redb backend, shared materializer/unitizer composition or live Qdrant integration.

## Verification targets

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test -p eliot-searchd --bin eliot-searchd --locked
cargo +1.98.0 test -p eliot-searchd --test persistent_roots_process --locked
```

Process tests invoke Cargo's actual `eliot-searchd` executable for registration/restart, explicit sync,
retained historical bytes after source changes, unavailable-root preservation, corrupt-state refusal
and public help. Unit tests cover catalog recovery, root overlap, swapped availability, exact paths,
owner exclusion and bounded control frames. These tests were added but not executed in the authoring
environment; no build, test, security or product acceptance is asserted here.
