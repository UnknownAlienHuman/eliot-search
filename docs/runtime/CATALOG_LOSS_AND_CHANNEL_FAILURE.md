# Missing catalogs and incomplete child responses

## Catalog loss

The primary `DirectStore::open` checks catalog presence before calling the legacy initializer.
`namespace.id` and `source-events.log` must either both exist as regular non-link files, or neither
may exist in a fresh root. One missing file returns `DIRECT_CONTROL_INCOMPLETE_RECOVERY_REQUIRED`.
With both absent, any revision-tree entry or unrecognized control residue blocks initialization.
Observation-root registration may precede first corpus creation and is retained.

`DirectStore::verify` and guarded GC require both catalog files without recreating either. Normal
log-chain and revision validation still follows: presence is not integrity. Malformed files are not
reset. Inventory validation now precedes opening the Windows secret protector.

These are read-only admission checks under the existing owner lock, not journal recovery. They do
not recover deleted history, make log append atomic, or prove final-handle containment against path
races. Explicit internal plaintext development constructors are unchanged. Completely erased state
with no surviving residue cannot be distinguished from a fresh root by this preflight.

## Incomplete proxy exchange

The proxy arms a non-reusable-channel fence before sending any command byte. Only consuming and
forwarding the complete response, including an ordinary command-error frame, releases the fence.
Write failure, child EOF, invalid framing or response-line exhaustion leaves the channel blocked.
The child is terminated without writing a shutdown command into a potentially blocked pipe.

The failure is `LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED`: dispatched mutations may have
committed. The proxy neither restarts the child nor replays the command. Its listener remains up but
refuses subsequent requests with `LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART` until the operator
restarts the proxy. Old stdout is never forwarded as the next client's response. This does not add
request deadlines, cancellation, or transactional recovery to the underlying source log.

## Verification

```sh
cargo +1.98.0 check --workspace --all-targets --all-features --locked
cargo +1.98.0 test --locked -p eliot-searchd --bin eliot-searchd --test catalog_loss_process
```

Seventeen regression cases were added (one Unix-only): six presence cases, six exchange cases and
five process cases using the primary binary. Process cases remove catalog files from disposable
indexed roots, invoke verification/GC/reindexing, and compare surviving revision bytes. Exchange
cases inject failed writes and incomplete responses; they are not live-socket integration tests.
The manual workflow's existing `core_tests` option includes the new process suite.

Rust compilation and tests were not executed: the attempted workspace check could not start because
`cargo` is absent, and toolchain downloads failed. Local lexical/delimiter, whitespace and exact-blob
checks are not substitutes for compilation. Windows stable-API replacement, uncertain log-commit
quarantine, single-owner experimental-runtime isolation, persistent redb composition, durable
preparation and live Qdrant remain outstanding. No acceptance or production-readiness claim is made.
