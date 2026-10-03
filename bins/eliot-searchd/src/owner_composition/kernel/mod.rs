//! Durable owner composition behind the stable daemon-local facade.

mod codec;
mod installation;
mod lifecycle;
mod observation;
mod record;
mod slots;
mod spec;
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
