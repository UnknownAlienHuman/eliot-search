//! Exact dependent dispatch over existing capability owners, not a second store.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_access::{AccessError, MAX_SECURITY_DEPENDENTS, SecurityDependentReceipt};
use search_contracts::{BoundedList, LiveDenySnapshotRef, OpaqueId, ReceiptRef};
use search_control_redb::security_restriction::SecurityRestrictionCommit;
use search_handles::HandleStore;
use search_ports::{CancellationProbe, OperationContext};

use super::{
    NativeSecurityBinding, NativeSecurityError, SecurityInvalidationSink,
    budget::MutationBudget, mapping,
};

mod handles;

/// Canonical handle owner's configuration identifier. This slot cannot be
/// registered as a generic callback or silently substituted with legacy tokens.
pub const HANDLE_SECURITY_DEPENDENT: &str = "search-handles";

/// One additional capability owner's executed, idempotent invalidation.
///
/// The registry supplies verified native operation and snapshot identities.
/// Implementations must return their own acknowledgement only after actual work,
/// honour the supplied cancellation/deadline, and preserve the same operation
/// across partial failures. They may not use a requested scope as proof of work.
/// The runtime must serialize these owners with the same security-domain lock.
pub trait SecurityDependentInvalidator<C: CancellationProbe> {
    /// Completes this owner's obligations for the exact full committed state.
    /// A returned reference is content-free execution evidence, not authority.
    fn invalidate(
        &mut self,
        committed: &SecurityRestrictionCommit,
        mutation_receipt: &ReceiptRef,
        published: &LiveDenySnapshotRef,
        context: &OperationContext<C>,
    ) -> Result<ReceiptRef, AccessError>;
}

enum RegisteredInvalidator<'a, C: CancellationProbe> {
    Handles(&'a mut HandleStore),
    Owner(&'a mut dyn SecurityDependentInvalidator<C>),
}

/// Finite dispatcher for the exact server-configured domain owner set.
///
/// Registration borrows real owners for the duration of native apply/recovery.
/// There is no default successful callback, skipped unknown owner or receipt
/// cache. Missing/duplicate/foreign registrations fail before any dependent is
/// called. A later failure returns no completion list; the native domain remains
/// closed and all owners must support replay of the same operation.
///
/// Construct under the existing domain lock, register the canonical handle store
/// with `register_handles` and every other required owner with `register_owner`,
/// then supply the registry to `NativeSecurityDomain::{restore, apply, recover}`.
/// Dropping the registry releases borrows, not security restrictions.
pub struct SecurityInvalidationRegistry<'a, C: CancellationProbe> {
    binding: &'a NativeSecurityBinding,
    owners: BTreeMap<OpaqueId, RegisteredInvalidator<'a, C>>,
}

impl<C: CancellationProbe> fmt::Debug for SecurityInvalidationRegistry<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityInvalidationRegistry")
            .field("registered", &self.owners.len())
            .field("required", &self.binding.dependents.len())
            .finish_non_exhaustive()
    }
}

impl<'a, C: CancellationProbe> SecurityInvalidationRegistry<'a, C> {
    /// Captures server configuration; no owner is inferred from a request.
    pub fn new(binding: &'a NativeSecurityBinding) -> Result<Self, AccessError> {
        if binding.dependents.is_empty() {
            return Err(AccessError::SecurityFailClosed);
        }
        Ok(Self { binding, owners: BTreeMap::new() })
    }

    /// Registers the actual canonical handle store. The implementation scans
    /// both complete restriction sets in bounded steps, including during startup
    /// recovery. No source bytes, token plaintext or whole-store clones are used.
    pub fn register_handles(&mut self, store: &'a mut HandleStore) -> Result<(), AccessError> {
        let owner = OpaqueId::new(HANDLE_SECURITY_DEPENDENT)
            .map_err(|_| AccessError::SecurityFailClosed)?;
        self.insert(owner, RegisteredInvalidator::Handles(store))
    }

    /// Registers another required owner. The canonical handle slot is reserved
    /// for the concrete implementation; arbitrary callbacks cannot replace it.
    pub fn register_owner(
        &mut self,
        owner: OpaqueId,
        invalidator: &'a mut dyn SecurityDependentInvalidator<C>,
    ) -> Result<(), AccessError> {
        if owner.as_str() == HANDLE_SECURITY_DEPENDENT {
            return Err(AccessError::SecurityOperationConflict);
        }
        self.insert(owner, RegisteredInvalidator::Owner(invalidator))
    }

    /// Checks complete registration without mutating any owner. Composition may
    /// call this before native apply; invalidation always checks it again.
    pub fn require_complete(&self) -> Result<(), AccessError> {
        if !self.owners.keys().eq(self.binding.dependents.iter()) {
            return Err(AccessError::SecurityFailClosed);
        }
        Ok(())
    }

    fn insert(
        &mut self,
        owner: OpaqueId,
        invalidator: RegisteredInvalidator<'a, C>,
    ) -> Result<(), AccessError> {
        if !self.binding.dependents.contains(&owner) || self.owners.contains_key(&owner) {
            return Err(AccessError::SecurityOperationConflict);
        }
        if self.owners.len() >= MAX_SECURITY_DEPENDENTS {
            return Err(AccessError::SecurityFailClosed);
        }
        self.owners.insert(owner, invalidator);
        Ok(())
    }
}

impl<C: CancellationProbe + Clone> SecurityInvalidationSink<C>
    for SecurityInvalidationRegistry<'_, C>
{
    fn invalidate(
        &mut self,
        committed: &SecurityRestrictionCommit,
        mutation_receipt: &ReceiptRef,
        published: &LiveDenySnapshotRef,
        context: &OperationContext<C>,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, AccessError> {
        let budget = MutationBudget::new(context).map_err(access_error)?;
        self.require_complete()?;
        mapping::validate_binding(self.binding, committed.mutation()).map_err(access_error)?;
        if mapping::receipt_ref(committed).map_err(access_error)? != *mutation_receipt
            || mapping::snapshot_ref(committed) != *published
        {
            return Err(AccessError::SecurityOperationConflict);
        }
        let mut received = BoundedList::empty();
        let mut references = BTreeSet::new();
        // BTreeMap iteration is the same canonical order as configured owners.
        // A single decreasing budget covers every owner, not one fresh timeout
        // per callback. Failed work is replayed, never declared completed here.
        for (owner, invalidator) in &mut self.owners {
            budget.check().map_err(access_error)?;
            let receipt_ref = match invalidator {
                RegisteredInvalidator::Handles(store) => {
                    handles::invalidate(store, committed, mutation_receipt, &budget)?
                }
                RegisteredInvalidator::Owner(invalidator) => invalidator.invalidate(
                    committed, mutation_receipt, published,
                    &budget.context().map_err(access_error)?,
                )?,
            };
            budget.check().map_err(access_error)?;
            if !references.insert(receipt_ref.clone()) {
                return Err(AccessError::SecurityOperationConflict);
            }
            received.try_push(SecurityDependentReceipt {
                dependent: owner.clone(),
                mutation_receipt_ref: mutation_receipt.clone(),
                live_snapshot_ref: published.clone(),
                receipt_ref,
            }).map_err(|_| AccessError::SecurityFailClosed)?;
        }
        mapping::validate_receipts(committed, published, &received).map_err(access_error)?;
        budget.check().map_err(access_error)?;
        Ok(received)
    }
}

fn access_error(error: NativeSecurityError) -> AccessError {
    match error {
        NativeSecurityError::Access(error) => error,
        _ => AccessError::SecurityFailClosed,
    }
}
