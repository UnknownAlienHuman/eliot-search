# Working on daemon root composition

[Root](../../../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md) owns exclusion/init/open;
[daemon](../../FUNCTIONS.md) owns composition. Use [local scope](../../../../docs/architecture/LOCAL_SCOPE.md),
not old owner-slot, key-selection or security-profile instructions.

Connect parsed settings → plain root/lock → actual stores/backend → handlers. Close in reverse lifetime order.
Do not put source registries, DEK selection, initialization journals, leases or command receipts behind another
root manager. Keep actual filesystem/data-integrity checks and useful OS helpers; no hostile-same-user sandbox.

Move callers and delete displaced wrappers together. Preserve user data and unpublished #598 work; it is not an
integration requirement. Minimal locked check/Clippy must include actual root/daemon/store consumers after code.
