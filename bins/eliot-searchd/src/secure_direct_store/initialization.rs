//! Borrowed protected initializer child, separate from every normal store opener.

use crate::owner_composition::InitializingDataRoot;
use crate::plaintext_direct_store as plaintext;
use crate::revision_protection::RevisionProtector;
use search_runtime_owner::OwnerError;

pub struct InitializationStore<'a> {
    root: &'a mut InitializingDataRoot,
    protector: RevisionProtector,
}

pub fn initialize_layout(
    root: &mut InitializingDataRoot,
) -> Result<InitializationStore<'_>, OwnerError> {
    plaintext::initialize_layout(root)?;
    let protector = RevisionProtector::for_initialization(root, true)?;
    root.checkpoint("protected-layout-open")?;
    Ok(InitializationStore { root, protector })
}

impl InitializationStore<'_> {
    pub(crate) fn finish(self) -> Result<[u8; 32], OwnerError> {
        plaintext::verify_initialization_layout(self.root, true)?;
        let selected = self.protector.initialization_key_binding()?;
        if existing_key_binding(self.root)? != selected {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        let Self { root, protector } = self;
        drop(protector);
        root.checkpoint("protected-child-closed")?;
        Ok(selected)
    }
}

pub fn existing_key_binding(root: &mut InitializingDataRoot) -> Result<[u8; 32], OwnerError> {
    plaintext::verify_initialization_layout(root, false)?;
    let protector = RevisionProtector::for_initialization(root, false)?;
    let binding = protector.initialization_key_binding()?;
    drop(protector);
    root.checkpoint("existing-protector-closed")?;
    Ok(binding)
}
