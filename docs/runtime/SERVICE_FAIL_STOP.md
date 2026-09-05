# Primary service failure boundaries

The primary `--serve-data-root` session arms a mutation attempt immediately before
`index-file`, `index-directory`, `sync-directory`, `retire`, or applied GC reaches
its storage operation. The attempt covers storage, subsequent verification and
response emission. Argument decoding happens first where available.

After dispatch, legacy string errors cannot prove that nothing changed. The
service therefore returns `SERVICE_MUTATION_OUTCOME_UNKNOWN`, invalidates local
handles and continuations, and exits nonzero without consuming another command
or emitting `data_root_stopped` with `clean=true`. It never retries the operation.
This conservative rule also stops on backend rejections that might actually have
had no effects; typed no-effect receipts can narrow that behavior later.

Output write/flush failure is latched. The session does not retry a partial JSON
frame, append a second error to it, or serve a queued request. A lost mutation
response remains outcome-unknown. Rejected input framing/UTF-8 also ends the
session. The shared line reader consumes at most the command ceiling plus two
framing bytes, rather than draining arbitrarily until a newline arrives.

Ordinary pre-dispatch validation errors with a complete error response remain
recoverable. Successful mutations permit the next command. Normal EOF/shutdown
retains the clean-stop event.

This is a process-local fail-stop barrier, not an atomic source-log transaction,
a persistent quarantine marker or a repair command. Restart must reopen and
verify storage; no rollback, automatic replay or torn-log repair is claimed.
General operation deadlines, durable recovery and redb cutover remain separate
unfinished work. The bounded reader is not a timeout for a silent client.

## Targets and verification

The two main packages expose only `eliot-searchd` and `eliot-search` as binary
targets. Six sealed prototypes and two snapshot programs are retained as explicit
`[[test]]` targets with `harness=true` and `test=true`. Their old CLI `main`
functions are not executed by the Rust test harness. This supersedes the proposed
runnable-example patch in `docs/execution/2026-09-05/T02_TARGET_ISOLATION.patch`:
runnable examples would still permit the conflicting legacy root owners.
No source or regression suite is deleted; all-target checking still sees them.
The full durable owner and capability extraction work remains unfinished.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd --test service_failure_process --test product_targets
cargo +1.98.0 test --locked -p eliot-searchd --test eliot-search-sealed-recover
```

Seven session unit tests, four primary-process regressions and two manifest
invariant tests cover the new boundaries. Process tests use disposable roots;
the catalog-loss fixture restores its own saved log explicitly. No fault switch
is added to the product. Rust compilation and test execution were unavailable
in the authoring environment; these commands are required, not passing evidence.
