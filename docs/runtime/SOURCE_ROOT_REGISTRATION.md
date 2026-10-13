# Persistent observation-root registration

The primary `eliot-searchd` selects source-root commands through the closed native command adapters.
They retain the original native/config invocation, deadline, cancellation and cumulative work under
existing-only root custody. Registration is not ingestion or permission to access source content.

The data root must already have an accepted initialized native layout. Ordinary commands never
initialize missing state. Source directories must be separate from it: neither a parent nor a child
of the data root is admitted. Stop any existing owner before running the one-shot commands.

```sh
eliot-searchd --register-source-root DATA_ROOT SOURCE_DIRECTORY
eliot-searchd --source-roots DATA_ROOT
eliot-searchd --sync-source-roots DATA_ROOT
eliot-searchd --search-root DATA_ROOT QUERY
eliot-searchd --unregister-source-root DATA_ROOT SOURCE_DIRECTORY
```

Registration is retained across process restarts. Unregistering stops this explicit observation path;
it neither revokes access to retained revisions nor purges them. Source IDs are not catalog positions.

## Current sync source contract — existing store, no effects

`--sync-source-roots DATA_ROOT` retains the registered list without additional source arguments.
Exactly two selected arguments are required after config selection; malformed arity keeps the fixed
`SOURCE_ROOT_USAGE_ERROR` frame before root admission. The original argv remains bound to the request.

The command acquires `OpenExistingMutating` custody but leaves the durable owner `RELEASED`. It borrows
the accepted registration pin, consumes its bounded bytes once and reuses the existing codec, path
policy and complete `SourceRootInspection`/`SourceRootCurrentness` observation. Zero roots, observation
gaps, missing/unsafe/non-directory/unverifiable roots and pending registration `.tmp`/`.bak` state
refuse. Only after successful complete preflight, the command reads existing namespace/source-log
bytes from held native pins, replays the existing catalog and loads the selected existing root
credential. Read and sync composition share the same replay and credential constructors.
A closed store/protector child exposes no effect or read surface. Its immutable root/selected-binding
checks and retained native/registration verification precede the terminal output attempt.

After valid preflight the command emits one bounded content-free refusal:

```json
{"error":"SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE","complete":false,"effects_may_have_committed":false}
```

This command traverses no source files, writes no storage/credential/lifecycle state and never
acknowledges currentness. Missing or mismatched existing state fails closed; no credential is created,
replaced or repaired. Persisted namespace/source-log/credential bytes remain compatible.
The command exits unsuccessfully.
A short/failed write, flush or post-output close checkpoint has unknown output outcome, cancels the
original operation and is never followed by another frame.
Store/protector closes before registration, then native pins/records/root custody; exclusions release
last. Contextual sync effects and successful lifecycle remain the next serial successor under #475.
The legacy create-or-open/effectful sync implementation remains only for deferred test fixtures.

## Failure semantics

The catalog has at most 32 non-overlapping absolute UTF-8 paths, each at most 512 bytes; the complete
file read is capped at 64 KiB. Exact path whitespace is preserved. Corrupt, truncated, duplicate or
unsupported catalog contents are rejected instead of being treated as an empty configuration. Symlink
and Windows reparse entries are rejected. Native handle-race qualification is still required.

Native registration/removal retains exact objects, stages bounded bytes, checks readback and publishes
under the original mutation context. Unknown publication preserves artifacts/fencing for its recovery
owner. Listing and sync preflight do not recover, replace or create a catalog. Windows power-loss
durability remains separately owned by #470; no qualification is asserted here.

No current sync refusal retires retained sources or proves an indexed/current workspace. Future
effectful sync must preserve partial/unknown effects, source-backed readback and truthful currentness;
the no-effect preflight is not a fabricated rollback or completed inventory receipt.

## Verification targets

```sh
cargo +1.98.0 check --locked --offline -p eliot-searchd --bin eliot-searchd
cargo +1.98.0 check --locked --offline -p eliot-searchd --bin eliot-searchd --all-features
cargo +1.98.0 clippy --locked --offline -p eliot-searchd --bin eliot-searchd --all-features -- -D warnings
```

Affected source guards cover real-command selection, original request/work, complete policy preflight,
one terminal attempt, old-route unreachability, no effects and child-before-parent teardown. Inherited
Clippy debt requires exact base/candidate attribution; new diagnostics block delivery.

Deferred #215 cases include config argv/arity, missing and unsafe roots, registration replacement and
pending sidecars, existing replay/credential mismatch or absence, preflight-before-credential ordering,
cancellation/deadline at each seam, short/failed output, release ordering and exact durable-state
equality before/after the command. Legacy effectful process fixtures remain historical
until separately updated for their owning successor. Source checks do not establish native, installed,
real-Qdrant or product qualification.
