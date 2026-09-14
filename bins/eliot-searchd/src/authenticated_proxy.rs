//! Authenticated loopback proxy for the owner-fenced DIRECT runtime.
//!
//! The stable daemon entry delegates to the bounded authenticated proxy
//! owners. Provider envelopes, child transport and endpoint key material stay
//! private to this composition boundary.

#[path = "authenticated_proxy/kernel.rs"]
mod kernel;

pub use kernel::maybe_run;
