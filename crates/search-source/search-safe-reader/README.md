# Safe reader

The current entrypoint is acquire_file. It opens the explicitly selected path once, checks native file identity, length, and change metadata on that handle, reads one bounded byte buffer, and verifies the same observations again before returning.

Unix requires device and inode identity. Windows requires volume serial and file index identity. Filesystems that cannot supply the required key return SOURCE_IDENTITY_UNAVAILABLE. The read has an 8 MiB ceiling and performs no source execution, network access, retry, path-parent admission, or identity fallback.

See [FUNCTIONS.md](FUNCTIONS.md) for the exact source-to-object caller path.
