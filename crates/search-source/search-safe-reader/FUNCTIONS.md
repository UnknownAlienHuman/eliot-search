# Safe reader — explicit source acquisition

## Current operation

| Symbol | Behavior |
|---|---|
| acquire_file | Opens one explicit path once without following the final reparse/symlink component, captures native identity/length/change metadata from the handle, reads at most 8 MiB into one buffer, then rechecks the same handle. |
| same_handle_read_core | Owns the bounded allocation, exact read, growth check, cancellation/deadline checks, and before/after observation comparison. |
| AcquiredFile::into_parts | Transfers the original acquired allocation together with the verified NativeFileKey and exact length. |

Windows opens read-only with FILE_SHARE_READ only, denying write/delete sharing for the handle's lifetime. An existing incompatible write/delete handle makes open fail as SOURCE_CHANGED_DURING_READ. FILE_FLAG_OPEN_REPARSE_POINT prevents following the final reparse target, and native attributes from the opened handle must not contain FILE_ATTRIBUTE_REPARSE_POINT. winapi-util queries volume serial and file index from that same File before and after the byte read; timestamps are supplementary observations.

Unix opens read-only with the native O_NOFOLLOW flag, retaining device/inode and mtime/ctime observations on the same handle. Other platforms return unsupported identity. If a required native ID is unavailable, acquisition fails. A truly empty file is a valid zero-byte source; open, read, identity, and stability failures never become empty success.

## Actual caller

bins/eliot-searchd native_root_command::retain_file holds the current PlainRootGuard, captures the current-source CAS guard, calls acquire_file once, moves the same byte allocation to search-revision-store, and commits the row only after retained-object readback.

Retained-object readback verifies the stored bytes and does not establish that acquisition was stable; acquisition owns its handle-open and before/after conditions independently.

No path parent is treated as an admitted root. No content is executed or decoded. The selected path is not reopened for retention. Source reconciliation and Git acquisition remain later work.

## Failures

SourceReadError reports a closed static code for missing/open failure, unsupported native identity, non-regular file, oversize input, allocation failure, changed handle, cancellation, deadline, or I/O. It carries no source path or body.
