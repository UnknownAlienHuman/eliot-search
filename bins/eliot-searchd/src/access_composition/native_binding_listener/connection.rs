//! Lifetime-bound opened and serving connection guards.

use core::fmt;
use std::task::Poll;

use crate::provider_composition::{
    CanonicalRecipeHost, CanonicalServingError, CanonicalServingLimits,
    CanonicalServingOwner, CanonicalTcpConnection,
};

use super::super::{
    BoundedStandaloneGrantIssuer, GrantEntropySource, GrantIssuerError,
    GrantTimeSource, GrantValidationClock, NativeBindingPin,
    SessionBoundGrantAuthority, StandaloneGrantIssuer, StandaloneGrantMaterial,
    StandaloneGrantPolicySource, StandaloneGrantRecipeHost, StandaloneGrantTemplate,
    StandaloneProcessOwner, StandaloneServingTransitionError,
};

/// One opened typed transport whose lifetime is bounded by the process owner.
/// Fields are private: the transport cannot be detached or stored independently.
pub struct StandaloneOpenedConnection<'a> {
    pub(super) transport: Option<CanonicalTcpConnection>,
    pub(super) pin: NativeBindingPin,
    pub(super) _process: &'a mut StandaloneProcessOwner,
}

impl<'a> StandaloneOpenedConnection<'a> {
    /// Exact current binding pin retained for live authority composition.
    #[must_use]
    pub const fn pin(&self) -> &NativeBindingPin {
        &self.pin
    }

    /// Execute the mandatory first grant command, then enter canonical serving.
    ///
    /// The issuer is borrowed by the one operation-scoped grant authority, so
    /// the exact ledger mutated by successful issuance is the ledger transferred
    /// into the serving host. There is no public ungranted transition and no way
    /// to replace the issuer between response delivery and recipe admission.
    ///
    /// Failure or unwind while reading, issuing or writing the grant response
    /// closes the sole transport. Serving construction happens only after the
    /// complete two-record authenticated response was written successfully.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_grant_and_into_standalone_serving<P, E, T, H>(
        mut self,
        host: H,
        policy_source: P,
        mut issuer: BoundedStandaloneGrantIssuer<E, T>,
        maximum_grant_deadline_ms: u64,
        limits: CanonicalServingLimits,
    ) -> Result<
        StandaloneServingConnection<'a, StandaloneGrantRecipeHost<E, T, H>>,
        StandaloneServingTransitionError,
    >
    where
        P: StandaloneGrantPolicySource,
        E: GrantEntropySource,
        T: GrantTimeSource + GrantValidationClock,
        H: CanonicalRecipeHost,
    {
        {
            let transport = self
                .transport
                .as_mut()
                .ok_or(CanonicalServingError::Closed)?;
            let mut authority = SessionBoundGrantAuthority::new(
                policy_source,
                BorrowedIssuer(&mut issuer),
            );
            let _claims = transport.issue_initial_standalone_grant(
                &mut authority,
                maximum_grant_deadline_ms,
            )?;
        }
        self.into_standalone_serving_after_grant(host, issuer, limits)
            .map_err(Into::into)
    }

    /// Transfer a successfully granted connection into the bounded serving owner.
    ///
    /// This is private so callers cannot bypass the mandatory first command or
    /// substitute an issuer ledger that did not mint the delivered claims.
    fn into_standalone_serving_after_grant<E, T, H>(
        mut self,
        host: H,
        issuer: BoundedStandaloneGrantIssuer<E, T>,
        limits: CanonicalServingLimits,
    ) -> Result<
        StandaloneServingConnection<'a, StandaloneGrantRecipeHost<E, T, H>>,
        CanonicalServingError,
    >
    where
        T: GrantValidationClock,
        H: CanonicalRecipeHost,
    {
        let transport = self
            .transport
            .take()
            .ok_or(CanonicalServingError::Closed)?;
        let serving = CanonicalServingOwner::<
            StandaloneGrantRecipeHost<E, T, H>,
        >::new_standalone(transport, host, issuer, limits)?;
        Ok(StandaloneServingConnection {
            serving,
            pin: self.pin,
            _process: self._process,
        })
    }

    /// Close without creating a recipe host, idempotently.
    pub fn close(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.close();
        }
    }
}

impl fmt::Debug for StandaloneOpenedConnection<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneOpenedConnection")
            .field("binding", &self.pin.record().binding_id)
            .field("open", &self.transport.is_some())
            .finish_non_exhaustive()
    }
}

struct BorrowedIssuer<'a, I>(&'a mut I);

impl<I: StandaloneGrantIssuer> StandaloneGrantIssuer for BorrowedIssuer<'_, I> {
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        self.0.issue(template)
    }
}

/// Canonical serving owner whose transport and tasks cannot outlive the root.
pub struct StandaloneServingConnection<'a, H: CanonicalRecipeHost> {
    serving: CanonicalServingOwner<H>,
    pin: NativeBindingPin,
    _process: &'a mut StandaloneProcessOwner,
}

impl<H: CanonicalRecipeHost> StandaloneServingConnection<'_, H> {
    /// Exact binding pin for host-side live registration/policy checks.
    #[must_use]
    pub const fn pin(&self) -> &NativeBindingPin {
        &self.pin
    }

    /// Service one bounded input/work turn.
    pub fn tick(&mut self) -> Result<(), CanonicalServingError> {
        self.serving.tick()
    }

    /// Run and normally drain until the caller's stop condition becomes true.
    /// On error the connection is closed but retained tasks remain available
    /// through `poll_cleanup`; dropping is fail-stop, never cleanup evidence.
    pub fn run(
        &mut self,
        stop: impl FnMut() -> bool,
    ) -> Result<(), CanonicalServingError> {
        self.serving.run(stop)
    }

    /// Stop admission/output and signal retained task resources.
    pub fn close(&mut self) {
        self.serving.close();
    }

    /// Advance at most one retained cleanup task after closure.
    pub fn poll_cleanup(&mut self) -> Result<Poll<()>, CanonicalServingError> {
        self.serving.poll_cleanup()
    }

    /// Queued/running/cleanup-pending task count.
    #[must_use]
    pub fn retained_tasks(&self) -> usize {
        self.serving.retained_tasks()
    }
}

impl<H: CanonicalRecipeHost> fmt::Debug for StandaloneServingConnection<'_, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneServingConnection")
            .field("binding", &self.pin.record().binding_id)
            .field("retained_tasks", &self.serving.retained_tasks())
            .finish_non_exhaustive()
    }
}
