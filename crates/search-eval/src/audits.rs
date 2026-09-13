//! Zero-tolerance leakage, admission, fault-recovery, protocol and reproducibility audits.
//!
//! Each audit surface has one private owner behind this stable public facade.

mod admission;
mod common;
mod fault;
mod leakage;
mod protocol;
mod reproducibility;

pub use admission::{AdmissionAudit, AdmissionProbe, AdmissionScenario, audit_source_admission};
pub use common::{HardBlocker, HardBlockerClass};
pub use fault::{
    FaultCell, FaultCellStatus, FaultContainment, FaultMatrixReport, FaultPoint, FaultReadback,
    audit_fault_matrix,
};
pub use leakage::{
    CanaryClass, LeakageAudit, LeakageObservation, LeakageSurface, audit_leakage,
};
pub use protocol::{ProtocolStressEvidence, ProtocolStressReport, audit_protocol_stress};
pub use reproducibility::{
    ReproducibilityObservation, ReproducibilityReport, audit_reproducibility,
};
