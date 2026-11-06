# Safe reader — explicit source acquisition

## Current operation

| Symbol | Behavior |
|---|---|
| acquire_file | Opens one explicit path once, captures native identity/length/change metadata from the handle, reads at most 8 MiB into one buffer, then rechecks the same handle. |
| same_handle_read_core | Owns the bounded allocation, exact read, growth check, cancellation/deadline checks, and before/after observation comparison. |
| AcquiredFile::into_parts | Transfers the original acquired allocation together with the verified NativeFileKey and exact length. |

Unix identity is device plus inode with mtime/ctime stability observations. Windows identity is volume serial plus file index with native write-time observations. If a required ID is unavailable, acquisition fails. A truly empty file is a valid zero-byte source; open, read, identity, and stability failures never become empty success.

## Actual caller

bins/eliot-searchd native_root_command::retain_file holds the current PlainRootGuard, captures the current-source CAS guard, calls acquire_file once, moves the same byte allocation to search-revision-store, and commits the row only after retained-object readback.

No path parent is treated as an admitted root. No content is executed or decoded. The selected path is not reopened for retention. Source reconciliation and Git acquisition remain later work.

## Failures

SourceReadError reports a closed static code for missing/open failure, unsupported native identity, non-regular file, oversize input, allocation failure, changed handle, cancellation, deadline, or I/O. It carries no source path or body.
