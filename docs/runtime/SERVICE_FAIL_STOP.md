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
Durable recovery and redb cutover remain unfinished. The raw stdio line reader
itself is not a timeout for a silent client.

## Loopback child and event boundaries

The existing `proxy_child::ChildIo` owns one child and one pipe worker. Its default
startup budget is 30 seconds; one 120-second request budget covers pipe writes,
response forwarding and shutdown exit/join. Cleanup has a separate five-second
budget. Channels have capacity one, frames remain limited to 64 KiB and aggregate
normal responses to 64 MiB. Progress does not reset the request deadline.
The child's exact shutdown frames are deferred until successful process exit.
OS termination failure is not proof of clean release; native containment and
canonical provider integration still have separate obligations.

The proxy now recognizes completion and ordinary errors through the child's
fixed event-first header, not substring searches anywhere in the line. A nested
`event` field or payload substring is not treated as the control signal.
Single-frame responses also require a recognizable event. This checks the
private producer's header shape, not the complete JSON grammar or a canonical
provider request ID. Full provider envelopes remain T19 work. Exact fatal service
frames still retain the exchange fence.

The unreferenced `owned_service.rs` alternative stdio implementation is removed.
`entry.rs` continues to use `public_runtime_service`; the existing bounded child
owner is preserved rather than replaced by another process controller.

## Targets and verification

The two main packages expose only `eliot-searchd` and `eliot-search` as binary
targets. Six sealed prototypes and two snapshot programs are retained as explicit
`[[test]]` targets with `harness=true` and `test=true`. Their old CLI `main`
functions are not executed by the Rust test harness. This supersedes the proposed
runnable-example patch in `docs/execution/2026-09-05/T02_TARGET_ISOLATION.patch`:
runnable examples would still permit the conflicting legacy root owners.
All eight retained harness targets and their regression suites remain available
in all-target checking. Full durable ownership and capability extraction remain unfinished.

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd --test service_failure_process --test product_targets
cargo +1.98.0 test --locked -p eliot-searchd --test eliot-search-sealed-recover
```

Existing session and primary-process regressions are retained. Process fixtures
use disposable roots; the catalog-loss fixture restores its saved log explicitly.
No compiler or native execution result is supplied for this increment; source
changes do not imply T06 or product acceptance.
