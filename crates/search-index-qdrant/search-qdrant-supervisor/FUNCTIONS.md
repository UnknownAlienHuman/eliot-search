# Qdrant process — source audit and replacement map

The application does not currently call this package. The useful first step is direct connection to a
configured endpoint through `search-qdrant-bridge`; managed process ownership is optional.

## Verified public-main source

| Source | Actual behavior / defect |
|---|---|
| `Cargo.toml` | Depends only on Eliot crates and `std`; no Windows API dependency can create a Job Object or apply an ACL. |
| `containment.rs` | Explicitly states that the package cannot call Job/ACL APIs. `evaluate_containment` accepts caller-supplied booleans, a digest and a receipt, then reports `contained=true`. This is a claim checker, not containment. |
| `artifact_policy.rs`, `identity.rs`, `sha256.rs` | Reimplement artifact/version/size/SHA and qualification records for one pinned build. This is release/evaluation metadata, not runtime process ownership. |
| `lib.rs` | Pure supervisor transition, restart/quarantine and receipt vocabulary. No normal daemon constructor was found. |
| `launch.rs::check_loopback_port_free` | Treats both `ConnectionRefused` and `TimedOut` as a free port. Timeout does not prove that no listener owns the endpoint. |
| `launch.rs::materialize_config_yaml` | Writes a fixed `config.yaml` with `std::fs::write`; it may overwrite existing state and has no unique stage/no-clobber/durability contract. |
| `launch.rs::spawn_qualified` | Inherits the daemon's full environment, adds the API key, creates fixed `run-stdout.log` / `run-stderr.log` in the data directory, then spawns. Logs are not bounded/rotated and unrelated parent environment is not sanitized. |
| `owned.rs` | Useful child-handle ownership and bounded poll/kill skeleton, but `Drop` only calls `kill()` and cannot report/reap. No child-tree/Job containment is applied. |
| `owned.rs::wait_ready` | Implements a second manual HTTP/1.0 client and body substring check instead of using the real Qdrant SDK/bridge. Each probe uses its own fixed timeout. |
| launch/readiness receipts | Store PID, executable/data-dir digests, ports and caller references. They are not needed to recover a child held by an actual process handle and do not connect publication/query. |
| callers | `spawn_qualified`, `QdrantSupervisor::new` and related construction were found in package tests/live fixtures, not product daemon composition. |

The package therefore contains real spawning code surrounded by authority it cannot establish and is not
used by the application.

## Target modes

### External/configured endpoint — baseline

```text
QdrantConnectionConfig {
  endpoint
  optional actual api_key
  connect_timeout
}
```

Daemon composition constructs the one bridge/runtime from this configuration. Startup performs a live
connection/version/capability observation and verifies the exact physical collection generation it will use.
No supervisor object, executable hash, secret lease or process receipt participates.

This mode is the required path for #263/#264. It works with a manually started Qdrant, installed service or
development process and keeps process management out of the first product slice.

### Managed child — optional convenience

Only after the baseline works, a selected profile may add:

```text
ManagedQdrantConfig {
  executable
  private data_dir
  private/generated config or explicit config path
  loopback endpoint
  optional actual api_key
  startup_timeout
  shutdown_timeout
}

ManagedQdrant {
  child_handle
  optional real platform child-tree guard
  endpoint
  owned temporary/config/log resources
}
```

The child wrapper is concrete, not a generalized lifecycle protocol.

## Managed launch sequence

```text
validate selected paths/ports/config
→ create private unique config if needed
→ prepare minimal sanitized environment
→ spawn exact child and retain handle
→ apply actual platform containment or fail managed mode
→ wait for bridge-authenticated live readiness under one absolute deadline
→ return endpoint + owned child
```

Rules:

1. Never decide that a port is free from connect timeout. Prefer OS-selected ports where supported or let
   bind fail and own cleanup; an occupied endpoint fails.
2. Never attach to an existing process. External mode connects to an endpoint; managed mode owns only the
   handle it spawned.
3. Do not hash/verify the executable on every start as runtime authority. Packaging/release may record an
   artifact checksum; startup may observe version for diagnostics/compatibility.
4. Do not write a fixed config/log file into a shared data directory. Use create-new/private paths and bounded
   capture or inherited/null output according to the selected diagnostics policy.
5. Sanitize inherited environment. Pass only required platform variables and selected Qdrant settings.
6. If authentication is enabled, one actual key is supplied to child and bridge. Rotation rebuilds the child
   or bridge as needed. No `SecretLeaseEvidence` or purpose digest.
7. Readiness uses the bridge/client and current endpoint credential. It verifies connectivity and the actual
   server behavior required by the selected profile.
8. No automatic restart in the first profile. Unexpected exit marks the dependency unavailable and preserves
   data. Later bounded restart is added only after a real availability requirement and measured benefit.
9. Shutdown uses the original absolute deadline. Stop application admission first, drop/close bridge users,
   terminate/reap the owned child/tree, then clean generated resources. Unknown teardown is reported; it does
   not create a durable receipt OS.

## Actual Windows containment

A Windows managed profile either:

- uses a real small platform adapter which creates/configures a Job Object, assigns the spawned child and
  provides kill-on-close/tree cleanup; or
- is unavailable.

`ContainmentEvidence { job_object_applied: true, digest, receipt }` is not an implementation. The actual Job
handle must remain owned with the child. If safe assignment requires a suspended spawn, implement that in the
platform adapter; do not pretend a Boolean closes the race.

## Code to keep or move

Potentially useful after simplification:

- `OwnedChild` around a real `std::process::Child`;
- bounded `try_wait` / kill / reap loops;
- literal loopback host validation;
- minimal private config rendering if managed mode generates config;
- content-free process errors.

Move these into daemon composition or a much smaller optional package. Reuse the Qdrant bridge for readiness.

## Delete after cutover

- artifact qualification manifest/candidate/compiled pin and package-local SHA implementation;
- owner fence / installation-incarnation / purpose / secret-lease evidence;
- caller-supplied containment report and launch/readiness/shutdown receipts;
- pure supervisor transition, restart decision, quarantine and operation-ID state;
- manual HTTP probe parser/client;
- path/PID/executable digest revalidation after actual child ownership;
- fixed log/config paths and exact old fixtures;
- P05/W3 wave and package-status requirements.

Delete the entire package if external mode remains the only shipped mode.

## Focused verification

External mode:

- endpoint unavailable/wrong version/auth failure;
- live server and exact selected collection schema;
- one absolute connect/request deadline;
- no process or qualification state created.

Optional managed mode:

- occupied port/bind failure;
- executable missing/exit before readiness;
- wrong/missing API key;
- actual Job/tree containment or honest unsupported result;
- startup timeout/cancellation owns child cleanup;
- crash marks dependency unavailable without deleting data;
- shutdown reaps child/tree and releases private files;
- environment/config/logs contain no unrelated secret material;
- no orphan child and no automatic restart loop.
