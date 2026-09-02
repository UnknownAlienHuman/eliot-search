//! Vendor-neutral operation ports and bounded process-local support types.
//!
//! This crate owns interface shape only. It does not choose an executor,
//! perform I/O, hold capability state, or expose vendor or native handles.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

pub mod access;
pub mod conformance;
pub mod context;
pub mod control;
pub mod error;
pub mod handles;
pub mod index;
pub mod optional;
pub mod preparation;
pub mod query;
pub mod runtime;
pub mod source;
pub mod stream;

pub use access::{AccessCheckpoint, SecurityPermitState};
pub use conformance::{
    ConformanceError, FakeCancellation, ForcedFailure, PORT_METHODS, PORTS, PortDescriptor,
    PortMethodDescriptor, ScriptStep, ScriptedOperation,
};
pub use context::{
    CancellationProbe, ContextReason, IdempotencyClass, MutationIdentity, OperationClass,
    OperationContext, PackageOpaque, Port, PortOutcome, PortReceipt, ReceiptRetryability,
};
pub use control::{ControlJournalPort, ControlSnapshotPort};
pub use error::{DisclosureClass, PortError, PortErrorKind, PortFailure, PortRetryability};
pub use handles::{HandleInvalidationScope, HandleLimits};
pub use index::{DefaultPinOwner, EpochPinPort, SearchIndexAdminPort, SearchIndexPort};
pub use optional::ModelProviderPort;
pub use preparation::{CodeEnricherPort, LexicalEncoderPort, MaterializerPort, UnitizerPort};
pub use query::{AccessCompilerPort, ExactScannerPort, HandleStorePort, OverlayPort};
pub use runtime::{ClockPort, MonotonicInstant, ProcessSupervisorPort, SecretStorePort};
pub use source::{
    ResidencyPolicyPort, SafeReaderPort, SourceAdmissionPort, SourceInventoryPort,
    SourceOwnershipPort, SourceRevisionStorePort,
};
pub use stream::{BoundedPage, BoundedStream, DEFAULT_PAGE_LIMIT, StreamTerminal};
