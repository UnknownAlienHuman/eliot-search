# PR #193: public runtime fail-stop session ownership

Date: 2026-09-20
Tracking: issue #189 / PR #193 / T02 service-composition phase

## Ownership correction

The primary DIRECT fail-stop line session was stored at the daemon source root as
`service_session.rs` and imported into `public_runtime_service` with a relative
`#[path]` escape. No other runtime owned or invoked it. Its tests were likewise
stored as the unrelated root-level `service_session_tests.rs`.

The session and its regression corpus now live under the sole consumer:

```text
public_runtime_service/kernel/session.rs
public_runtime_service/kernel/session/tests.rs
```

`public_runtime_service/kernel.rs` uses an ordinary private `mod session;`.
The obsolete root-level files are deleted.

## Preserved semantics

This is a relocation and ownership cleanup only. The session still:

- uses the existing bounded `protocol_io::read_line` adapter;
- terminates after command limit, oversize, invalid UTF-8, timeout or read I/O
  failure;
- permits validation failures to return one error and continue only when no
  mutation was dispatched and output remained complete;
- classifies every error after a dispatched mutation as
  `SERVICE_MUTATION_OUTCOME_UNKNOWN` without exposing the private cause;
- latches the first partial/zero/flush output failure and never consumes another
  command;
- treats persistent catalog quarantine as fatal even before a mutation effect;
- leaves data-root acquire, drain, clean release and search-state invalidation in
  runtime composition.

No protocol frame, reason code, persisted state, dependency, workflow, gate or
launch-state entry changes.

## Boundary decision

The session was not moved to `search-runtime-owner`: that package owns the
process/data-root lease, owner epoch, drain and clean release, but explicitly
owns no daemon request composition. It was also not added to
`search-provider-protocol`: this legacy DIRECT line shell is not the generic
mutually authenticated provider protocol and must not create a second request
lifecycle owner there.

Keeping the fail-stop loop private to `public_runtime_service` preserves one
owner while removing the cross-directory module escape.

## Regression coverage

The relocated tests continue to cover:

- validation error followed by healthy commands;
- mutation failure without next-command consumption or private error disclosure;
- acknowledged mutation followed by an independent validation error;
- bounded oversize read without draining a following command;
- invalid UTF-8 termination;
- write, zero-write and flush failure latching;
- mutation output failure classified outcome-unknown;
- quarantine termination;
- silent-client timeout and connection-reset classification.

The module ownership test now requires the session and its tests inside the
public runtime kernel and requires both obsolete root-level files to remain
absent.

## Required execution

```text
cargo test --locked -p eliot-searchd public_runtime_service::kernel::session::tests
cargo test --locked -p eliot-searchd \
  --test public_runtime_service_module_ownership
cargo check --locked -p eliot-searchd --all-targets --all-features
cargo fmt --all -- --check
cargo clippy --locked -p eliot-searchd --all-targets --all-features \
  -- -D warnings
```

Execution status in the current authoring environment: **NOT_RUN** (`cargo`,
`rustc`, and `rustfmt` are absent). No runtime qualification, T02 acceptance or
independent review is claimed.
