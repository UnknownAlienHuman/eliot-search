//! T29 route-rebuild, epoch-pin and safe-reclamation composition.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! retained truth, rebuild planning/readback, verified route cutover,
//! process-local pin lifecycle and ordinary exact-ID reclaim authorization.
//! Backend transport remains caller-owned; this module contains no Qdrant SDK
//! or network runtime type and never treats orphan backend state as current.

#![forbid(unsafe_code)]

#[path = "rebuild_composition/kernel.rs"]
mod kernel;

pub use kernel::*;
