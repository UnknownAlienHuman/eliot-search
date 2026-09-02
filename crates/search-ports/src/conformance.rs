//! Exact port inventory and finite conformance fakes.
use crate::{CancellationProbe, MutationIdentity, PackageOpaque};
use core::fmt;
use search_contracts::{BoundedList, ContractError, OpaqueId};

/// One exact shared port descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortDescriptor {
    /// Public Rust trait name.
    pub name: &'static str,
    /// Package-local module that owns the trait.
    pub module: &'static str,
    /// Exact number of registered operations.
    pub method_count: usize,
}

/// One exact shared port method descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortMethodDescriptor {
    /// Public Rust trait name.
    pub port: &'static str,
    /// Exact operation name.
    pub method: &'static str,
    /// Capability-owner logical module for the operation.
    pub implementation_module: &'static str,
}

/// Exact closed registry of all shared vendor-neutral ports.
pub const PORTS: [PortDescriptor; 23] = [
    PortDescriptor {
        name: "ClockPort",
        module: "runtime",
        method_count: 2,
    },
    PortDescriptor {
        name: "SecretStorePort",
        module: "runtime",
        method_count: 4,
    },
    PortDescriptor {
        name: "ProcessSupervisorPort",
        module: "runtime",
        method_count: 5,
    },
    PortDescriptor {
        name: "ControlJournalPort",
        module: "control",
        method_count: 6,
    },
    PortDescriptor {
        name: "ControlSnapshotPort",
        module: "control",
        method_count: 1,
    },
    PortDescriptor {
        name: "SourceAdmissionPort",
        module: "source",
        method_count: 3,
    },
    PortDescriptor {
        name: "SourceInventoryPort",
        module: "source",
        method_count: 5,
    },
    PortDescriptor {
        name: "SourceOwnershipPort",
        module: "source",
        method_count: 3,
    },
    PortDescriptor {
        name: "SafeReaderPort",
        module: "source",
        method_count: 3,
    },
    PortDescriptor {
        name: "SourceRevisionStorePort",
        module: "source",
        method_count: 5,
    },
    PortDescriptor {
        name: "ResidencyPolicyPort",
        module: "source",
        method_count: 3,
    },
    PortDescriptor {
        name: "MaterializerPort",
        module: "preparation",
        method_count: 2,
    },
    PortDescriptor {
        name: "UnitizerPort",
        module: "preparation",
        method_count: 2,
    },
    PortDescriptor {
        name: "CodeEnricherPort",
        module: "preparation",
        method_count: 2,
    },
    PortDescriptor {
        name: "LexicalEncoderPort",
        module: "preparation",
        method_count: 4,
    },
    PortDescriptor {
        name: "ModelProviderPort",
        module: "optional",
        method_count: 3,
    },
    PortDescriptor {
        name: "SearchIndexPort",
        module: "index",
        method_count: 7,
    },
    PortDescriptor {
        name: "SearchIndexAdminPort",
        module: "index",
        method_count: 2,
    },
    PortDescriptor {
        name: "EpochPinPort",
        module: "index",
        method_count: 4,
    },
    PortDescriptor {
        name: "AccessCompilerPort",
        module: "query",
        method_count: 4,
    },
    PortDescriptor {
        name: "OverlayPort",
        module: "query",
        method_count: 3,
    },
    PortDescriptor {
        name: "ExactScannerPort",
        module: "query",
        method_count: 2,
    },
    PortDescriptor {
        name: "HandleStorePort",
        module: "query",
        method_count: 5,
    },
];

/// Exact closed registry of all shared port operations.
pub const PORT_METHODS: [PortMethodDescriptor; 80] = [
    PortMethodDescriptor {
        port: "ClockPort",
        method: "utc_now",
        implementation_module: "adapters",
    },
    PortMethodDescriptor {
        port: "ClockPort",
        method: "monotonic_now",
        implementation_module: "adapters",
    },
    PortMethodDescriptor {
        port: "SecretStorePort",
        method: "create_secret",
        implementation_module: "store",
    },
    PortMethodDescriptor {
        port: "SecretStorePort",
        method: "lease_secret",
        implementation_module: "lease",
    },
    PortMethodDescriptor {
        port: "SecretStorePort",
        method: "rotate_secret",
        implementation_module: "rotation",
    },
    PortMethodDescriptor {
        port: "SecretStorePort",
        method: "delete_secret",
        implementation_module: "store",
    },
    PortMethodDescriptor {
        port: "ProcessSupervisorPort",
        method: "qualify_artifact",
        implementation_module: "artifact",
    },
    PortMethodDescriptor {
        port: "ProcessSupervisorPort",
        method: "start_process",
        implementation_module: "process",
    },
    PortMethodDescriptor {
        port: "ProcessSupervisorPort",
        method: "verify_process_identity",
        implementation_module: "identity",
    },
    PortMethodDescriptor {
        port: "ProcessSupervisorPort",
        method: "readiness",
        implementation_module: "health",
    },
    PortMethodDescriptor {
        port: "ProcessSupervisorPort",
        method: "shutdown_process",
        implementation_module: "shutdown",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "read_control_snapshot",
        implementation_module: "snapshot",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "transact",
        implementation_module: "transaction",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "compare_and_swap_visible_epoch",
        implementation_module: "transaction",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "load_unresolved_publication",
        implementation_module: "transaction",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "quarantine",
        implementation_module: "quarantine",
    },
    PortMethodDescriptor {
        port: "ControlJournalPort",
        method: "write_counters",
        implementation_module: "transaction",
    },
    PortMethodDescriptor {
        port: "ControlSnapshotPort",
        method: "current_snapshot",
        implementation_module: "snapshot",
    },
    PortMethodDescriptor {
        port: "SourceAdmissionPort",
        method: "normalize_policy",
        implementation_module: "policy",
    },
    PortMethodDescriptor {
        port: "SourceAdmissionPort",
        method: "evaluate",
        implementation_module: "decision",
    },
    PortMethodDescriptor {
        port: "SourceAdmissionPort",
        method: "verify_receipt",
        implementation_module: "receipt",
    },
    PortMethodDescriptor {
        port: "SourceInventoryPort",
        method: "resolve_source_view",
        implementation_module: "view",
    },
    PortMethodDescriptor {
        port: "SourceInventoryPort",
        method: "resolve_workspace_view",
        implementation_module: "view",
    },
    PortMethodDescriptor {
        port: "SourceInventoryPort",
        method: "list_exact_denominator",
        implementation_module: "view",
    },
    PortMethodDescriptor {
        port: "SourceInventoryPort",
        method: "lookup_source_head",
        implementation_module: "source",
    },
    PortMethodDescriptor {
        port: "SourceInventoryPort",
        method: "read_inventory_revision",
        implementation_module: "snapshot",
    },
    PortMethodDescriptor {
        port: "SourceOwnershipPort",
        method: "read_namespace_owner",
        implementation_module: "cutover",
    },
    PortMethodDescriptor {
        port: "SourceOwnershipPort",
        method: "transition_namespace_owner",
        implementation_module: "cutover",
    },
    PortMethodDescriptor {
        port: "SourceOwnershipPort",
        method: "verify_cutover_receipt",
        implementation_module: "cutover",
    },
    PortMethodDescriptor {
        port: "SafeReaderPort",
        method: "resolve_final_source",
        implementation_module: "platform",
    },
    PortMethodDescriptor {
        port: "SafeReaderPort",
        method: "stable_read",
        implementation_module: "stable_read",
    },
    PortMethodDescriptor {
        port: "SafeReaderPort",
        method: "read_git_object_no_execute",
        implementation_module: "git_object",
    },
    PortMethodDescriptor {
        port: "SourceRevisionStorePort",
        method: "admit_revision",
        implementation_module: "revision",
    },
    PortMethodDescriptor {
        port: "SourceRevisionStorePort",
        method: "reopen_exact",
        implementation_module: "revision",
    },
    PortMethodDescriptor {
        port: "SourceRevisionStorePort",
        method: "retain",
        implementation_module: "lease",
    },
    PortMethodDescriptor {
        port: "SourceRevisionStorePort",
        method: "release_retention",
        implementation_module: "lease",
    },
    PortMethodDescriptor {
        port: "SourceRevisionStorePort",
        method: "enumerate_mark_roots",
        implementation_module: "lifecycle",
    },
    PortMethodDescriptor {
        port: "ResidencyPolicyPort",
        method: "resolve_residency",
        implementation_module: "residency",
    },
    PortMethodDescriptor {
        port: "ResidencyPolicyPort",
        method: "authorize_copy_or_reencrypt",
        implementation_module: "residency",
    },
    PortMethodDescriptor {
        port: "ResidencyPolicyPort",
        method: "record_transition",
        implementation_module: "residency",
    },
    PortMethodDescriptor {
        port: "MaterializerPort",
        method: "profile",
        implementation_module: "profile",
    },
    PortMethodDescriptor {
        port: "MaterializerPort",
        method: "materialize",
        implementation_module: "product",
    },
    PortMethodDescriptor {
        port: "UnitizerPort",
        method: "profile",
        implementation_module: "profile",
    },
    PortMethodDescriptor {
        port: "UnitizerPort",
        method: "unitize",
        implementation_module: "manifest",
    },
    PortMethodDescriptor {
        port: "CodeEnricherPort",
        method: "profile",
        implementation_module: "profile",
    },
    PortMethodDescriptor {
        port: "CodeEnricherPort",
        method: "enrich",
        implementation_module: "facts",
    },
    PortMethodDescriptor {
        port: "LexicalEncoderPort",
        method: "profile",
        implementation_module: "profile",
    },
    PortMethodDescriptor {
        port: "LexicalEncoderPort",
        method: "encode_document",
        implementation_module: "sparse",
    },
    PortMethodDescriptor {
        port: "LexicalEncoderPort",
        method: "encode_query",
        implementation_module: "sparse",
    },
    PortMethodDescriptor {
        port: "LexicalEncoderPort",
        method: "fixture_digest",
        implementation_module: "fixture",
    },
    PortMethodDescriptor {
        port: "ModelProviderPort",
        method: "profile",
        implementation_module: "profile",
    },
    PortMethodDescriptor {
        port: "ModelProviderPort",
        method: "encode",
        implementation_module: "encode",
    },
    PortMethodDescriptor {
        port: "ModelProviderPort",
        method: "rerank",
        implementation_module: "rerank",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "probe_capabilities",
        implementation_module: "capability",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "ensure_schema",
        implementation_module: "schema",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "upsert_exact",
        implementation_module: "mutation",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "close_exact",
        implementation_module: "mutation",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "readback_exact",
        implementation_module: "readback",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "query",
        implementation_module: "query",
    },
    PortMethodDescriptor {
        port: "SearchIndexPort",
        method: "exact_count",
        implementation_module: "readback",
    },
    PortMethodDescriptor {
        port: "SearchIndexAdminPort",
        method: "delete_exact",
        implementation_module: "admin",
    },
    PortMethodDescriptor {
        port: "SearchIndexAdminPort",
        method: "validate_route",
        implementation_module: "admin",
    },
    PortMethodDescriptor {
        port: "EpochPinPort",
        method: "acquire_epoch_pin",
        implementation_module: "acquire",
    },
    PortMethodDescriptor {
        port: "EpochPinPort",
        method: "acquire_route_pin",
        implementation_module: "acquire",
    },
    PortMethodDescriptor {
        port: "EpochPinPort",
        method: "reclamation_watermark",
        implementation_module: "watermark",
    },
    PortMethodDescriptor {
        port: "EpochPinPort",
        method: "release_owner",
        implementation_module: "release",
    },
    PortMethodDescriptor {
        port: "AccessCompilerPort",
        method: "validate_grant",
        implementation_module: "grant",
    },
    PortMethodDescriptor {
        port: "AccessCompilerPort",
        method: "intersect_scope",
        implementation_module: "scope",
    },
    PortMethodDescriptor {
        port: "AccessCompilerPort",
        method: "compile_safe_legs",
        implementation_module: "legs",
    },
    PortMethodDescriptor {
        port: "AccessCompilerPort",
        method: "revalidate_checkpoint",
        implementation_module: "checkpoint",
    },
    PortMethodDescriptor {
        port: "OverlayPort",
        method: "snapshot_overlay",
        implementation_module: "snapshot",
    },
    PortMethodDescriptor {
        port: "OverlayPort",
        method: "shadowed_memberships",
        implementation_module: "shadow",
    },
    PortMethodDescriptor {
        port: "OverlayPort",
        method: "direct_candidates",
        implementation_module: "direct",
    },
    PortMethodDescriptor {
        port: "ExactScannerPort",
        method: "compile_exact_scan",
        implementation_module: "plan",
    },
    PortMethodDescriptor {
        port: "ExactScannerPort",
        method: "execute_exact_scan",
        implementation_module: "execute",
    },
    PortMethodDescriptor {
        port: "HandleStorePort",
        method: "mint_ephemeral",
        implementation_module: "issue",
    },
    PortMethodDescriptor {
        port: "HandleStorePort",
        method: "mint_durable",
        implementation_module: "issue",
    },
    PortMethodDescriptor {
        port: "HandleStorePort",
        method: "expand",
        implementation_module: "expand",
    },
    PortMethodDescriptor {
        port: "HandleStorePort",
        method: "invalidate",
        implementation_module: "invalidate",
    },
    PortMethodDescriptor {
        port: "HandleStorePort",
        method: "expire",
        implementation_module: "cleanup",
    },
];

/// Forced conformance outcome supported by package-local fake ports.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ForcedFailure {
    /// Deadline expired before dispatch.
    DeadlineBeforeStart,
    /// Deadline expired while work was active.
    DeadlineDuringOperation,
    /// Cancellation arrived before any side effect.
    CancelledBeforeSideEffect,
    /// Cancellation arrived after an acknowledged side effect.
    CancelledAfterSideEffect,
    /// Dependency generation or guarded CAS was stale.
    StaleGenerationOrCas,
    /// Operation completed a bounded partial result.
    Partial,
    /// Required dependency was unavailable.
    DependencyUnavailable,
    /// Same mutation identity may be replayed idempotently.
    SameIdentityReplay,
    /// Different identity retry is rejected as unsafe.
    UnsafeDifferentIdentityRetry,
}

/// One finite scripted fake step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptStep<T, E> {
    /// Return a successful bounded value.
    Return(T),
    /// Return a typed package-local error.
    Fail(E),
    /// Force one cross-port conformance condition.
    Force(ForcedFailure),
}

/// Failure in the conformance-script harness itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConformanceError {
    /// No scripted step remains.
    ScriptExhausted,
    /// A mutation script requires a mutation identity.
    MissingMutationIdentity,
    /// Retry used a different immutable operation identity.
    MutationIdentityChanged,
}

/// Finite deterministic operation script.
///
/// The script is bounded at construction, has no queue or background worker,
/// and can bind all mutation retries to one exact operation identity.
#[derive(Clone, Debug)]
pub struct ScriptedOperation<T, E, const LIMIT: usize> {
    steps: BoundedList<ScriptStep<T, E>, LIMIT>,
    cursor: usize,
    bound_operation_id: Option<OpaqueId>,
}

impl<T, E, const LIMIT: usize> ScriptedOperation<T, E, LIMIT>
where
    T: Clone,
    E: Clone,
{
    /// Creates a finite read-only script.
    ///
    /// # Errors
    ///
    /// Returns a contract bound error when `steps` exceeds `LIMIT`.
    pub fn new(steps: Vec<ScriptStep<T, E>>) -> Result<Self, ContractError> {
        Ok(Self {
            steps: BoundedList::new(steps)?,
            cursor: 0,
            bound_operation_id: None,
        })
    }

    /// Creates a finite mutation script bound to one exact operation identity.
    ///
    /// # Errors
    ///
    /// Returns a contract bound error when `steps` exceeds `LIMIT`.
    pub fn for_mutation(
        mutation: &MutationIdentity,
        steps: Vec<ScriptStep<T, E>>,
    ) -> Result<Self, ContractError> {
        Ok(Self {
            steps: BoundedList::new(steps)?,
            cursor: 0,
            bound_operation_id: Some(mutation.operation_id.clone()),
        })
    }

    /// Returns the number of unconsumed finite steps.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.steps.len().saturating_sub(self.cursor)
    }

    /// Consumes the next deterministic step after mutation-identity checking.
    ///
    /// # Errors
    ///
    /// Returns a harness error for exhaustion, missing identity, or unsafe
    /// different-identity retry.
    pub fn next(
        &mut self,
        mutation: Option<&MutationIdentity>,
    ) -> Result<ScriptStep<T, E>, ConformanceError> {
        if let Some(expected) = &self.bound_operation_id {
            let Some(actual) = mutation else {
                return Err(ConformanceError::MissingMutationIdentity);
            };
            if &actual.operation_id != expected {
                return Err(ConformanceError::MutationIdentityChanged);
            }
        }
        let Some(step) = self.steps.as_slice().get(self.cursor).cloned() else {
            return Err(ConformanceError::ScriptExhausted);
        };
        self.cursor = self.cursor.saturating_add(1);
        Ok(step)
    }
}

/// Minimal redacted cancellation fake for port conformance tests.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FakeCancellation {
    cancelled: bool,
}

impl FakeCancellation {
    /// Creates a fake cancellation state.
    #[must_use]
    pub const fn new(cancelled: bool) -> Self {
        Self { cancelled }
    }

    /// Changes the fake cancellation state.
    pub const fn set_cancelled(&mut self, cancelled: bool) {
        self.cancelled = cancelled;
    }
}

impl fmt::Debug for FakeCancellation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FakeCancellation(<opaque>)")
    }
}

impl PackageOpaque for FakeCancellation {
    fn owner_package(&self) -> &'static str {
        "search-ports"
    }
}

impl CancellationProbe for FakeCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use search_contracts::OpaqueId;

    use super::{
        ConformanceError, ForcedFailure, PORT_METHODS, PORTS, ScriptStep, ScriptedOperation,
    };
    use crate::{IdempotencyClass, MutationIdentity};

    #[test]
    fn machine_inventory_is_exact_and_unique() {
        assert_eq!(PORTS.len(), 23);
        assert_eq!(PORT_METHODS.len(), 80);
        assert_eq!(
            PORTS.iter().map(|port| port.method_count).sum::<usize>(),
            PORT_METHODS.len()
        );
        let ports = PORTS.iter().map(|port| port.name).collect::<BTreeSet<_>>();
        assert_eq!(ports.len(), PORTS.len());
        let methods = PORT_METHODS
            .iter()
            .map(|entry| (entry.port, entry.method))
            .collect::<BTreeSet<_>>();
        assert_eq!(methods.len(), PORT_METHODS.len());
        assert!(PORT_METHODS.iter().all(|entry| ports.contains(entry.port)));
    }

    #[test]
    fn scripted_operation_is_finite() {
        let mut script = ScriptedOperation::<u8, (), 2>::new(vec![
            ScriptStep::Return(1),
            ScriptStep::Force(ForcedFailure::Partial),
        ])
        .expect("bounded script");
        assert_eq!(script.remaining(), 2);
        assert_eq!(script.next(None), Ok(ScriptStep::Return(1)));
        assert_eq!(
            script.next(None),
            Ok(ScriptStep::Force(ForcedFailure::Partial))
        );
        assert_eq!(script.next(None), Err(ConformanceError::ScriptExhausted));
    }

    #[test]
    fn mutation_script_accepts_same_identity_and_rejects_different_identity() {
        let first = MutationIdentity::new(
            OpaqueId::new("operation:first").expect("id"),
            IdempotencyClass::RetrySameIdentity,
        );
        let other = MutationIdentity::new(
            OpaqueId::new("operation:other").expect("id"),
            IdempotencyClass::RetrySameIdentity,
        );
        let mut script = ScriptedOperation::<(), (), 2>::for_mutation(
            &first,
            vec![
                ScriptStep::Force(ForcedFailure::SameIdentityReplay),
                ScriptStep::Return(()),
            ],
        )
        .expect("bounded script");
        assert!(script.next(Some(&first)).is_ok());
        assert_eq!(
            script.next(Some(&other)),
            Err(ConformanceError::MutationIdentityChanged)
        );
        assert_eq!(script.remaining(), 1);
    }
}
