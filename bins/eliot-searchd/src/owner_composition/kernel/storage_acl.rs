//! Original-context ACL qualification over held storage handles only.

use std::{fs::File, path::Path, time::Instant};

use eliot_searchd::native_file::{
    StorageAclObservation, StorageSecurityContext, StorageSecurityWork,
    create_owner_private_directory, observe_owner_private_acl,
};
use search_os_secrets_windows::{
    self as secrets, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork, RevisionDekContext,
};
use search_runtime_owner::OwnerError;

use super::{
    native_objects::{NativePins, OBJECTS},
    native_request::{NativeRootRequest, NativeWork},
};

// The mutable original meter is borrowed, never replaced or reset. Leaf cleanup
// may still account an actual native clear after ordinary work has refused.
struct OriginalContext<'a> {
    request: &'a NativeRootRequest,
    work: &'a mut NativeWork,
    first: Option<OwnerError>,
}

impl<'a> OriginalContext<'a> {
    const fn new(request: &'a NativeRootRequest, work: &'a mut NativeWork) -> Self {
        Self {
            request,
            work,
            first: None,
        }
    }

    fn remember<T>(&mut self, result: Result<T, OwnerError>) -> Result<T, OwnerError> {
        match result {
            Err(error) => Err(*self.first.get_or_insert(error)),
            Ok(value) => self.first.map_or(Ok(value), Err),
        }
    }

    fn checkpoint(&mut self) -> Result<(), OwnerError> {
        let result = self.request.check();
        self.remember(result)
    }

    fn native_call(&mut self) -> Result<(), OwnerError> {
        if let Some(error) = self.first {
            return Err(error);
        }
        let result = self.work.call(self.request, || Ok(()));
        self.remember(result)
    }

    fn open(&mut self) -> Result<(), OwnerError> {
        if let Some(error) = self.first {
            return Err(error);
        }
        let result = self.work.open(self.request);
        self.remember(result)?;
        // charge_open/Open reserves the opening attempt; NativeCall separately
        // covers the leaf's additional ABI work and prepaid mandatory release.
        self.native_call()
    }

    fn bytes(&mut self, count: usize) -> Result<(), OwnerError> {
        if let Some(error) = self.first {
            return Err(error);
        }
        let result = self.work.read(self.request, count);
        self.remember(result)
    }

    fn finish<T, E>(&mut self, result: Result<T, E>, refusal: OwnerError) -> Result<T, OwnerError> {
        match result {
            Ok(value) => {
                self.checkpoint()?;
                Ok(value)
            }
            Err(_) => Err(self.first.unwrap_or(refusal)),
        }
    }
}

impl StorageSecurityContext for OriginalContext<'_> {
    type Error = OwnerError;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.checkpoint()
    }
    fn charge(&mut self, work: StorageSecurityWork) -> Result<(), Self::Error> {
        match work {
            StorageSecurityWork::NativeCall => self.native_call(),
            StorageSecurityWork::Open => self.open(),
            StorageSecurityWork::Allocate(bytes) | StorageSecurityWork::Process(bytes) => {
                self.bytes(bytes)
            }
        }
    }
}

impl LegacyRevisionDpapiContext for OriginalContext<'_> {
    type Error = OwnerError;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.checkpoint()
    }
    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        match work {
            LegacyRevisionDpapiWork::NativeCall => self.native_call(),
            LegacyRevisionDpapiWork::Allocate(bytes) | LegacyRevisionDpapiWork::Process(bytes) => {
                self.bytes(bytes)
            }
        }
    }
    fn account_native_clear(&mut self, actual_bytes: usize) -> Result<(), Self::Error> {
        let result = self.work.account_native_clear(self.request, actual_bytes);
        self.remember(result)
    }
}

impl RevisionDekContext for OriginalContext<'_> {
    fn deadline(&self) -> Instant {
        self.request.credential_context().0
    }
    fn charge_open(&mut self) -> Result<(), Self::Error> {
        self.open()
    }
}

fn actual_user(context: &mut OriginalContext<'_>) -> Result<[u8; 32], OwnerError> {
    context.checkpoint()?;
    let result = secrets::observe_revision_user(context);
    context
        .finish(result, OwnerError::OwnerStorageAclUnqualified)
        .map(|user| user.user_scope_digest())
}

fn observe(
    context: &mut OriginalContext<'_>,
    file: &File,
    user: [u8; 32],
) -> Result<StorageAclObservation, OwnerError> {
    context.checkpoint()?;
    let result = observe_owner_private_acl(file, user, context);
    context.finish(result, OwnerError::OwnerStorageAclUnqualified)
}

/// Captured current-user identity and qualification of the existing initializer root.
/// This owns no handle and grants no permission without a fresh held-parent check.
pub(super) struct RootOwnerSeed {
    user: [u8; 32],
    root: StorageAclObservation,
}

pub(super) fn qualify_existing_root(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    root: &File,
) -> Result<RootOwnerSeed, OwnerError> {
    let mut context = OriginalContext::new(request, work);
    let user = actual_user(&mut context)?;
    let root = observe(&mut context, root, user)?;
    Ok(RootOwnerSeed { user, root })
}

impl RootOwnerSeed {
    /// Freshly qualifies each newly opened held file before its first content write,
    /// including the initialization journal and the DB before redb construction.
    /// Reuses the actual user observation; private root ACLs alone prove no inheritance.
    pub(super) fn qualify_file(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        file: &File,
    ) -> Result<(), OwnerError> {
        observe(&mut OriginalContext::new(request, work), file, self.user).map(|_| ())
    }

    /// Freshly qualifies the actual held root, then creates with the leaf's
    /// explicit private descriptor. Never rewrites or adopts an existing child.
    pub(super) fn create_directory(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        path: &Path,
        actual_parent: &File,
    ) -> Result<(), OwnerError> {
        let mut context = OriginalContext::new(request, work);
        let parent = observe(&mut context, actual_parent, self.user)?;
        if parent != self.root {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        let result = create_owner_private_directory(path, &parent, &mut context);
        context.finish(result, OwnerError::OwnerAcquireOutcomeUnknown)
    }

    /// Reuses the initializer's actual user capture rather than reopening tokens.
    pub(super) fn capture_snapshot(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        pins: &NativePins,
    ) -> Result<StorageAclSnapshot, OwnerError> {
        let snapshot = StorageAclSnapshot::capture_user(request, work, pins, self.user)?;
        if snapshot.observations.first() != Some(&self.root) {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        Ok(snapshot)
    }
}

/// In-memory ordered ACL observations, never a persisted authority or native proof cache.
pub(super) struct StorageAclSnapshot {
    user: [u8; 32],
    observations: Vec<StorageAclObservation>,
    direct: bool,
}

impl StorageAclSnapshot {
    pub(super) fn capture(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        pins: &NativePins,
    ) -> Result<Self, OwnerError> {
        let user = actual_user(&mut OriginalContext::new(request, work))?;
        Self::capture_user(request, work, pins, user)
    }

    /// Qualifies the actual held DB file before redb acquisition may mutate it.
    /// This opens no token and does not itself extend the pinned snapshot.
    pub(super) fn qualify_file(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        file: &File,
    ) -> Result<StorageAclObservation, OwnerError> {
        observe(&mut OriginalContext::new(request, work), file, self.user)
    }

    /// Extends an eleven-pin snapshot only after the DIRECT DB pin is installed.
    /// Fences both the original pins and the exact pre-constructor DB ACL facts.
    pub(super) fn capture_after_direct_pin(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        pins: &NativePins,
        pre_open_direct: &StorageAclObservation,
    ) -> Result<Self, OwnerError> {
        if self.direct || pins.direct.is_none() || self.observations.len() != OBJECTS.len() + 1 {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        let extended = Self::capture_user(request, work, pins, self.user)?;
        if extended.observations[..self.observations.len()] != self.observations[..]
            || extended.observations.last() != Some(pre_open_direct)
        {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        Ok(extended)
    }

    fn capture_user(
        request: &NativeRootRequest,
        work: &mut NativeWork,
        pins: &NativePins,
        user: [u8; 32],
    ) -> Result<Self, OwnerError> {
        if pins.files.len() != OBJECTS.len() + 1 {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let count = pins.files.len() + usize::from(pins.direct.is_some());
        work.read(
            request,
            count
                .checked_mul(std::mem::size_of::<StorageAclObservation>())
                .ok_or(OwnerError::ContractExhausted)?,
        )?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(count)
            .map_err(|_| OwnerError::ContractExhausted)?;
        let mut context = OriginalContext::new(request, work);
        for file in pins
            .files
            .iter()
            .chain(pins.direct.iter().map(super::native_objects::DirectPin::borrow_file))
        {
            observations.push(observe(&mut context, file, user)?);
        }
        Ok(Self {
            user,
            observations,
            direct: pins.direct.is_some(),
        })
    }

    pub(super) fn verify(
        &self,
        request: &NativeRootRequest,
        work: &mut NativeWork,
        pins: &NativePins,
    ) -> Result<(), OwnerError> {
        if pins.files.len() != OBJECTS.len() + 1
            || pins.direct.is_some() != self.direct
            || self.observations.len() != pins.files.len() + usize::from(self.direct)
        {
            return Err(OwnerError::OwnerStorageAclUnqualified);
        }
        let mut context = OriginalContext::new(request, work);
        for (file, expected) in pins
            .files
            .iter()
            .chain(pins.direct.iter().map(super::native_objects::DirectPin::borrow_file))
            .zip(&self.observations)
        {
            if observe(&mut context, file, self.user)? != *expected {
                return Err(OwnerError::OwnerStorageAclUnqualified);
            }
        }
        context.checkpoint()
    }
}
