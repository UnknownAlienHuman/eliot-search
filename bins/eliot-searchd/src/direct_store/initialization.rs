//! Initialization-only legacy layout bytes; no ordinary store or migration open.

use super::model::SOURCE_LOG_HEADER;
use crate::owner_composition::InitializingDataRoot;
use crate::sha256;
use search_runtime_owner::OwnerError;

pub fn initialize_layout(root: &mut InitializingDataRoot) -> Result<(), OwnerError> {
    root.create_layout_directory(3)?;
    root.create_layout_directory(4)?;
    let namespace = format!("{}\n", sha256::hex(&root.namespace())).into_bytes();
    root.publish_layout_file(8, &namespace)?;
    root.publish_layout_file(9, format!("{SOURCE_LOG_HEADER}\n").as_bytes())?;
    verify_layout(root, true)
}

pub fn verify_layout(root: &mut InitializingDataRoot, empty: bool) -> Result<(), OwnerError> {
    let namespace = format!("{}\n", sha256::hex(&root.namespace())).into_bytes();
    if root.layout_bytes(8, 65)? != namespace {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let header = format!("{SOURCE_LOG_HEADER}\n").into_bytes();
    if empty {
        if root.layout_bytes(9, header.len())? != header {
            return Err(OwnerError::OwnerGuardMismatch);
        }
        root.require_empty_revisions()?;
    } else if root.layout_prefix(9, &header)? != header {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    root.checkpoint("layout-verified")
}
