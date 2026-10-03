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
use spec::{
    INSTALLATION_FILE, LifecycleState, MAX_STATE_BYTES, Slot,
};
#[cfg(test)]
use succession::verify_sealed_head_agrees;

#[cfg(test)]
mod tests;
