//! Single durable installation and runtime owner protocol for one data root.
//!
//! The stable daemon-local ownership surface delegates to bounded private
//! owners for persisted records, installation identity, native observation,
//! alternating slots, guarded succession and live drain/release lifecycle.

#[path = "owner_composition/kernel.rs"]
mod kernel;
pub use kernel::{NativeRegistrationOperation, SourceDirectoryMetadata, run_source_registration};

#[cfg(test)]
pub use kernel::ShutdownReceipt;
pub use kernel::{LiveOwner, establish};

pub use kernel::InitializingDataRoot;
#[cfg(test)]
pub use kernel::initialize_new;
pub use kernel::{initialize_direct, maybe_run_direct_registry};
pub use kernel::{NativeReadOperation, NativeReadStartup, RetainedShutdown};
pub use kernel::{NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing};
#[allow(unused_imports)] // Used by the dormant native process constructor.
pub(crate) use kernel::NativeProcessRootCustody;
pub use kernel::{NativeSyncPreflightOperation, run_source_sync_preflight};

#[cfg(all(test, windows))]
pub(crate) use kernel::fixture_initialization_namespace;
