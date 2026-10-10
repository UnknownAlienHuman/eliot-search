//! Durable owner composition behind the stable daemon-local facade.

#[path = "kernel/catalog_intent.rs"]
mod catalog_intent;
#[path = "kernel/catalog_intent_decode.rs"]
mod catalog_intent_decode;
#[path = "kernel/catalog_recovery.rs"]
mod catalog_recovery;

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/initialization.rs"]
mod initialization;
#[path = "kernel/inspection.rs"]
mod inspection;
#[path = "kernel/installation.rs"]
mod installation;
#[path = "kernel/lifecycle.rs"]
mod lifecycle;
#[path = "kernel/native_bindings.rs"]
mod native_bindings;
#[path = "kernel/observation.rs"]
mod observation;
#[path = "kernel/operation.rs"]
mod operation;
#[path = "kernel/read_existing.rs"]
mod read_existing;
#[path = "kernel/record.rs"]
mod record;
#[path = "kernel/slots.rs"]
mod slots;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/succession.rs"]
mod succession;

pub(crate) use initialization::{
    InitializationRecovery, InitializationRequest, InitializingDataRoot, initialize_new_request,
    recover_initialization_request,
};
pub(crate) use inspection::{ExistingOwnerSnapshot, inspect_existing_owner};
pub(crate) use installation::{retain_native_installation, verify_native_installation};
pub use lifecycle::{LiveOwner, ShutdownReceipt};
pub(crate) use native_bindings::NativeLayoutPins;
pub(crate) use operation::DataRootRequest;
pub(crate) use catalog_intent::CatalogMutationIntent;
pub(crate) use catalog_recovery::{
    CatalogRecoveryInspection, CatalogRecoveryRequest, inspect_catalog_recovery_request,
    list_catalog_recovery_request,
};
pub(crate) use read_existing::{
    open_bound_directory, verify_bound_directory, verify_existing_locator,
};
#[cfg(test)]
pub use succession::establish;
pub(crate) use succession::establish_existing;

#[cfg(test)]
use codec::hex;
#[cfg(test)]
use observation::{observe_executable, observe_physical_root};
#[cfg(test)]
use record::DurableOwnerRecord;
#[cfg(test)]
use spec::{INSTALLATION_FILE, LifecycleState, MAX_STATE_BYTES, Slot};
#[cfg(test)]
use succession::verify_sealed_head_agrees;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
