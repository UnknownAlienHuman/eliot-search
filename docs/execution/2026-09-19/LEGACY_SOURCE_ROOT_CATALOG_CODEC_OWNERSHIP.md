# PR #193: legacy source-root catalog codec ownership

Date: 2026-09-19
Tracking: issue #189 / PR #193 / T02 source-root ownership phase

## Ownership move

The primary daemon previously owned both concrete filesystem publication and the
exact persisted schema of `control/source-roots.v1`:

```text
# ELIOT Search source roots v1
<absolute UTF-8 root locator>
...
```

`search-source-registry` now owns the frozen pure compatibility contract:

- exact versioned header;
- newline framing and mandatory final newline;
- UTF-8 decoding;
- duplicate locator rejection;
- 32-root ceiling;
- 64 KiB encoded-file ceiling;
- byte-exact encoding with caller order preserved;
- typed package-local codec failures.

The package treats locator lines as opaque text. It performs no filesystem I/O,
path canonicalization, overlap/containment policy, symlink/reparse checks, source
identity derivation, root probing, durable publication, or crash recovery.

## Daemon composition retained

`eliot-searchd::source_roots` retains:

- bounded exact file reads and metadata stability checks;
- platform `PathBuf` conversion and absolute/UTF-8/control/parent-component
  validation;
- canonicalization and root/data-root overlap policy;
- symlink/reparse rejection;
- temporary/backup recovery;
- create-new temporary write, sync, rename, readback and outcome-unknown
  classification;
- live root probes, watcher hints, reconciliation generations and currentness
  gaps.

The daemon translates package codec failures to the historical reasons:

- too many roots → `SOURCE_ROOT_LIMIT`;
- oversized file → `SOURCE_ROOT_CONFIG_TOO_LARGE`;
- non-UTF-8 → `SOURCE_ROOT_CONFIG_NOT_UTF8`;
- header/newline/duplicate framing failures → `SOURCE_ROOT_CATALOG_CORRUPT`.

Platform path failures remain classified by the existing daemon path validator,
so absolute-path, UTF-8, length and control-character behavior is unchanged.

## Compatibility

Unchanged:

- catalog filename `control/source-roots.v1`;
- exact header and line order;
- trailing newline;
- root/file/path ceilings;
- temporary and backup names;
- recovery and publication sequence;
- migration input bytes;
- command responses and reason codes;
- Cargo dependencies and `Cargo.lock`;
- workflows, gate and launch state.

## Regression coverage

Package tests freeze empty and multi-root golden bytes, caller-order
preservation, duplicate rejection, malformed header/final newline, non-UTF-8,
and finite bounds.

Daemon ownership tests require the schema literal and codec implementation to
remain in `search-source-registry`, require daemon spec aliases to the package
bounds, and retain filesystem/path/currentness owners in their existing modules.
The existing source-root process corpus continues to cover persistence, restart,
malformed-current recovery, overlap, missing roots, watcher overflow and sync
proof invalidation.

## Required execution

```text
cargo test --locked -p search-source-registry legacy_root_catalog
cargo test --locked -p eliot-searchd --test source_roots_module_ownership
cargo test --locked -p eliot-searchd source_roots::kernel::tests
cargo check --locked -p search-source-registry -p eliot-searchd \
  --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p search-source-registry -p eliot-searchd \
  --all-targets --all-features -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No native Windows qualification, T02
acceptance, or independent review is claimed.
