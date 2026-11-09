# Vendored ignore 0.4.34

Imported from the published crates.io `ignore` 0.4.34 package. The package's
SHA-256 is `484be5657551d7868d67587df967fb4fcbe9d266a761cc855aaa148fda0b3cc7`,
verified against the local Cargo archive and the original workspace lock entry.
Upstream repository: https://github.com/BurntSushi/ripgrep, commit
`91f5d22ef48bde459d90a8c7dbeab416e97d0f2b`, path `crates/ignore`.

The published source, manifests, examples, tests, benchmarks, README and license
files are retained. Cargo's registry cache markers and the package's separate
Cargo.lock are omitted. `COPYING`, `LICENSE-MIT` and `UNLICENSE` preserve the
upstream Unlicense OR MIT terms. Version and dependency declarations are unchanged.

## Exact source change

Only `src/dir.rs`, `src/gitignore.rs` and `src/lib.rs` differ from the retained published files.
[ELIOT_VENDOR.patch](ELIOT_VENDOR.patch) is the complete diff against those three published files, with
portable relative paths and zero context. From the verified upstream source directory, apply it with
`git apply --unidiff-zero ELIOT_VENDOR.patch` to reproduce the modified files. Applying it to the
checksum-verified upstream sources reproduces the vendored content byte for byte; every other published
file is unchanged apart from the repository's own line-ending normalization.

ELIOT's patch changes automatic ignore-loading error propagation only:

- Missing optional files (`NotFound`) remain optional. Other read/stat failures
  are retained instead of being dropped as generic I/O errors.
- Directory and parent loading retain their existing parser and precedence.
  Optional-file probes no longer use `Path::exists` or `Path::is_file` to hide
  access failures, including repository markers and worktree `commondir` reads.
- Global config/global ignore read errors reach the root walk result through the
  existing error channel instead of being logged or discarded.

The existing daemon rejects iterator errors and attached `DirEntry::error`
before accepting a complete scan, and supplies an explicitly obtained CWD so
global rules cannot be silently disabled by CWD discovery failure.
No replacement parser, traversal, cache,
worker, matching policy or new dependency is introduced.
