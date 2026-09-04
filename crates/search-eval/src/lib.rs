#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod audits;
mod core;
mod error;
mod evidence;
mod fingerprint;
mod metrics;
mod report;

pub use audits::*;
pub use core::*;
pub use error::EvalError;
pub use evidence::*;
pub use metrics::*;
pub use report::*;

pub(crate) use fingerprint::FingerprintBuilder;
