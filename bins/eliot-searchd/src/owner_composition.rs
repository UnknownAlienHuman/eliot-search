//! Single durable installation and runtime owner protocol for one data root.
//!
//! The stable daemon-local ownership surface delegates to bounded private
//! owners for persisted records, installation identity, native observation,
//! alternating slots, guarded succession and live drain/release lifecycle.

#[path = "owner_composition/kernel.rs"]
mod kernel;

#[cfg(test)]
pub use kernel::establish;
pub(crate) use kernel::establish_existing;
pub(crate) use kernel::verify_native_installation;
pub(crate) use kernel::{ExistingOwnerSnapshot, inspect_existing_owner};
pub(crate) use kernel::{
    InitializationRecovery, InitializationRequest, InitializingDataRoot, initialize_new,
    recover_initialization,
};
pub use kernel::{LiveOwner, ShutdownReceipt};
pub(crate) use kernel::{NativeLayoutPins, retain_native_installation};
pub(crate) use kernel::{open_bound_directory, verify_bound_directory, verify_existing_locator};
