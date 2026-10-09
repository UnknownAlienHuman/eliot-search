//! The closed S9.5 payload and S10.3 eligibility contract.
//!
//! These values describe one immutable collection generation; they perform no
//! provider I/O and issue no source, publication or access authority. Bridge
//! translation consumes this contract. Exact source validation remains required.

mod codec;
mod collection;
mod eligibility;
mod fields;
mod payload;
mod readback;
mod vectors;

pub use collection::*;
pub use eligibility::*;
pub use fields::*;
pub use payload::*;
pub use readback::*;
pub use vectors::*;
