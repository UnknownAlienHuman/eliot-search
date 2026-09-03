//! Deterministic, typed, redacted, I/O-free configuration mechanics.
//!
//! Input acquisition and capability-specific runtime application remain outside
//! this package. Callers supply already captured file bytes, environment values,
//! CLI values, accepted profiles, and package-owned validation digests.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod action;
pub mod diff;
pub mod document;
pub mod environment;
pub mod error;
pub mod fingerprint;
pub mod limits;
pub mod merge;
pub mod names;
pub mod plan;
pub mod redact;
pub mod registry;
pub mod section;
pub mod snapshot;
pub mod source;
pub mod value;

pub use action::{ReceiptKind, ReconfigurationAction, ReloadClass};
pub use diff::{ConfigDelta, SectionDelta, diff};
pub use document::{ConfigDocument, parse_document};
pub use environment::validate_environment_key;
pub use error::ConfigError;
pub use fingerprint::{ConfigFingerprint, SectionFingerprintInput, fingerprint};
pub use limits::ConfigLimits;
pub use merge::{ConfigLayers, FieldProvenance, MergedConfig, MergedField, merge_layers};
pub use names::{ConfigKeyName, ConfigKeyPath, ConfigOwner, ConfigSectionName, ConfigSourceRef};
pub use plan::{ReconfigurationPlan, ReconfigurationStep, plan_reconfiguration};
pub use redact::{
    DisclosureLevel, PathLocationClass, RedactedConfigView, RedactedValue, redacted_view,
};
pub use registry::{
    ConfigFieldDescriptor, ConfigRegistry, ConfigSectionDescriptor, SecretPolicy, register_sections,
};
pub use section::{ConfigSectionField, ConfigSectionInput, ValidatedSection, project_section};
pub use snapshot::{EffectiveConfigSnapshot, EffectiveSection, assemble_effective};
pub use source::{ConfigSource, ConfigSourceKind};
pub use value::{
    ConfigValue, ConfigValueKind, DocumentValue, LayerOperation, RedactionPolicy, SecretReference,
    SecurityFloor, ValueBounds,
};
