//! Durable owner composition behind the stable daemon-local facade.

#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/installation.rs"]
mod installation;
#[path = "kernel/lifecycle.rs"]
mod lifecycle;
#[path = "kernel/observation.rs"]
mod observation;
#[path = "kernel/record.rs"]
mod record;
#[path = "kernel/slots.rs"]
mod slots;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/succession.rs"]
mod succession;

pub use lifecycle::{LiveOwner, ShutdownReceipt};
pub use succession::establish;

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

#[path = "kernel/native_admission.rs"]
mod native_admission;
#[path = "kernel/native_objects.rs"]
mod native_objects;
#[path = "kernel/native_request.rs"]
mod native_request;
#[path = "kernel/direct_policy_v2.rs"]
mod direct_policy_v2;
#[path = "kernel/native_snapshot.rs"]
mod native_snapshot;
pub use std::fs::Metadata as SourceDirectoryMetadata;
#[path = "kernel/registration_native.rs"]
mod registration_native;
pub use native_admission::{NativeReadOperation, NativeReadStartup, RetainedShutdown};
pub use native_admission::{NativeRegistrationOperation, run_source_registration};
pub use native_admission::{NativeRootAdmission, admit_existing};
pub use native_admission::{NativeSyncPreflightOperation, run_source_sync_preflight};
pub use native_request::{NativeRootLimits, NativeRootRequest};

#[path = "kernel/initialization.rs"]
mod initialization;
#[path = "kernel/initialization_journal.rs"]
mod initialization_journal;
pub use initialization::InitializingDataRoot;
#[cfg(test)]
pub use initialization::initialize_new;
pub use initialization::initialize_direct;
pub use native_admission::direct_registry::maybe_run as maybe_run_direct_registry;
