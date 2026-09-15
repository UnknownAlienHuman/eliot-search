# PR #191 / issue #187: bounded repository scan increment

Date: 2026-09-15. Base: `388e8b1b9b06fc22a286684115627d62b00c6cb5`.
Scope: the integration-owned Qdrant structural validator and its documentation.
The tracking PR remains open; this increment is not a qualification receipt.

## Corrections

The previous validator recursively enumerated the repository without an
aggregate entry/depth bound, read matching UTF-8 files without an explicit byte
budget, and silently skipped symbolic links. A sufficiently large or linked
working tree could therefore produce an incomplete scan without a clear causal
failure.

This increment adds one scan budget shared by enumeration and every manifest,
Rust source, lockfile, qualification-source and artifact-manifest read:

- maximum directory depth: 64;
- maximum observed entries: 200,000;
- maximum bytes from one text file: 16 MiB;
- maximum text bytes in one validation run: 256 MiB.

Observed symlinks and unsupported filesystem entry types now produce stable
errors. The root itself is checked before traversal. Read paths are rechecked
before opening, opened objects must still be regular files, and a file that
grows beyond its allowance is rejected. Limit exhaustion stops further work and
cannot be rendered as `PASS`.

The exclusions remain explicit and narrow: `.git`, `target`, `.venv`,
`__pycache__` and `node_modules`. No network, subprocess, repository mutation,
Qdrant process, authority record or new dependency is introduced.

## Regression inventory

Four file-local unit tests cover entry exhaustion, directory-depth exhaustion,
per-file and aggregate byte exhaustion, and symlink rejection on Unix. Existing
public-validator synthetic-repository tests remain unchanged and continue to
exercise deterministic, read-only repeated validation.

## Verification boundary

The selected files were assembled against the exact main base and inspected for
bounded failure paths. Rust compilation, tests, rustfmt, strict Clippy, Windows
execution and live Qdrant qualification are **NOT_RUN** because the execution
environment has no Rust toolchain. No runtime `PASS`, accepted handoff or
independent review is claimed.

Outstanding #191 work remains compiler/exact-head execution and independent
review. Macro expansion is intentionally outside this lexical validator; a
compiled public-API check remains complementary rather than being imitated by a
home-grown Rust parser.
