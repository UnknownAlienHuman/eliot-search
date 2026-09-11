# Control cutover marker ownership

`search-control-redb::migration` owns the canonical `control-cutover.v1` marker schema, strict codec and replay classification.

The owner API is deliberately I/O-free:

- `ControlCutoverMarker` encodes and decodes the exact twelve-line marker;
- `classify_control_cutover_replay` separates identical replay, divergent history and a foreign installation/data-root owner;
- exact marker file name, staged database schema and byte ceiling are exported from the same owner;
- malformed identities, non-canonical digests, path-like locators, zero epochs, wrong schemas and torn bytes fail closed.

`eliot-searchd` remains the integration owner for the live effects around this record: data-root exclusion, legacy catalog readback, staging, temp/write/sync/rename publication, quarantine arming, status projection and serving-path composition. The daemon must not redefine the marker wire format or replay semantics.

This move does not complete T10/T11, activate redb as the serving authority, delete the legacy journal or prove crash-safe cutover. The committed marker still reports `serve_path_reroute: pending-integration-wiring` until the actual single-authority cutover is wired and qualified.

Manual checks:

```powershell
cargo test --locked -p search-control-redb cutover
cargo test --locked -p eliot-searchd --test control_migration_owner_boundary
```
