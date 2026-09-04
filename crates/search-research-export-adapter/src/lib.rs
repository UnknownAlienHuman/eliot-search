//! Deterministic research-export boundary for ELIOT Search.
//!
//! This crate owns finite export accounting, evidence/gap separation and
//! truthful finalization. Concrete serialization, filesystem writes and remote
//! publication remain adapter responsibilities outside this package.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod export;

pub use export::{
    BuilderState, ExportCoverage, ExportError, ExportFormat, ExportLimits, ExportManifest,
    ExportPolicy, ResearchExport, ResearchExportBuilder, ResearchGap, ResearchItem,
};
