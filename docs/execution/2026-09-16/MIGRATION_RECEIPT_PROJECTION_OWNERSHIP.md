# Migration receipt and status projection ownership

Date: 2026-09-16  
Tracking: issue #189 / PR #193 / T02 control-migration ownership

## Result

The canonical content-minimized projections for staged source migration and
control cutover now belong to `search-control-redb::migration`.

The package owns the exact field order, spelling, boolean semantics, locator
projection and lower-case digest encoding for:

- `source_migration_plan_staged`;
- `control_cutover_committed`;
- `control_cutover_rollback`;
- `control_cutover_status`.

The corresponding typed package surfaces are:

```text
SourceMigrationPlanLocation
SourceMigrationStagedPlan::render_json
ControlCutoverStatusState
ControlCutoverStatusProjection::render_json
render_control_cutover_committed_receipt
render_control_cutover_rollback_receipt
```

These projections consume already verified technical state. They do not read a
source body, acquire a data-root owner, publish a marker, activate redb, arm
quarantine, infer missing authority or switch a serving path.

## Daemon composition after the move

`eliot-searchd` retains the integration work that cannot enter the control
package:

- qualified data-root and native-file observations;
- source-registry replay and retained-object verification;
- owner/incarnation/epoch checks;
- catalog-quarantine policy;
- staged database presence/length observation;
- call ordering across staging, marker publication and source-snapshot recheck.

The daemon no longer defines `StagedPlan`, `staged_plan_json`,
`cutover_receipt`, rollback JSON or status JSON. It builds package-owned typed
state and asks the package projection to render the historical response.

## Compatibility

The operator-visible JSON remains byte-compatible with the previous daemon
implementation:

- staged-plan locator prefixes remain `""`, `control/migration-plans/` and
  `control/` for the three closed location families;
- `explicit_output_directory` versus `data_root` labels are unchanged;
- the staged database schema remains `source-map-content-v2`;
- cutover receipts still state `serve_path_reroute` as
  `pending-integration-wiring`;
- rollback remains a verified no-op over the preserved file journal;
- status remains read-only and reports the same `file-journal`,
  `marker-corrupt` and `redb-control-marker-v1` authority states;
- no source text, query content, credentials or unrestricted paths were added.

No dependency, lockfile, persisted database format, marker bytes, gate state or
qualification receipt changed.

## Regression seams

Package tests freeze:

- all staged-plan location families;
- staged database and content-manifest locator rendering;
- cutover marker locator/digest and owner fields;
- replay and database-reuse booleans;
- byte-exact rollback output;
- absent, corrupt and committed cutover-status states;
- complete/read-only/quarantine semantics;
- content minimization.

`control_migration_owner_boundary` requires these schemas and renderers to stay
in `search-control-redb::migration/receipts.rs`. It rejects restoration of the
receipt strings, local `StagedPlan`, `staged_plan_json`, `cutover_receipt` or
status schema in daemon composition.

## Remaining ownership work

The source-import mapping, content-manifest, immutable record artifacts,
inactive redb output lifecycle, cutover marker filesystem lifecycle and their
projections are now package-owned. Remaining issue #189 work moves the accepted
T02 direct-store/source-root/materialization clusters in their documented order
and wires the committed redb marker into the actual primary control serve path.
The latter is a behavioral integration task, not permission to delete the
preserved file journal or infer authority from marker presence alone.

## Required execution

```text
cargo +1.98.0 test --locked -p search-control-redb migration::receipts
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_owner_boundary
cargo +1.98.0 test --locked -p eliot-searchd --test control_migration_process
cargo +1.98.0 check --locked -p search-control-redb -p eliot-searchd --all-targets
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --locked -p search-control-redb -p eliot-searchd --all-targets -- -D warnings
```

Execution status in the current environment: **NOT_RUN**. `cargo`, `rustc` and
`rustfmt` are unavailable. No compile, test, format, Clippy, process-fixture,
T02 or independent-review PASS is claimed.
