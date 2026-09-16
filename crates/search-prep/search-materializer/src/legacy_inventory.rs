//! Pure physical-inventory grammar for legacy DIRECT preparation artifacts.
//!
//! Filesystem traversal and metadata observation remain daemon composition.
//! This owner closes the admitted tree names, final/temporary filename grammar,
//! classification tags and canonical relative locators used by migration
//! inspection.

use crate::legacy_store::{
    LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_OBJECTS_DIRECTORY,
    LEGACY_PREPARATION_REFERENCES_DIRECTORY, LegacyPreparationProtection,
    legacy_preparation_object_file_name,
    legacy_preparation_reference_file_name, legacy_preparation_shard,
};

/// Maximum one-file basename admitted by the legacy preparation inventory.
pub const LEGACY_PREPARATION_MAX_INVENTORY_NAME_BYTES: usize = 192;

/// Closed physical tree immediately below the preparation root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyPreparationInventoryTree {
    /// Lookup-reference tree.
    References,
    /// Encoded manifest-object tree.
    Objects,
}

impl LegacyPreparationInventoryTree {
    /// Canonical directory name.
    #[must_use]
    pub const fn directory_name(self) -> &'static str {
        match self {
            Self::References => LEGACY_PREPARATION_REFERENCES_DIRECTORY,
            Self::Objects => LEGACY_PREPARATION_OBJECTS_DIRECTORY,
        }
    }

    /// Parses exactly one admitted direct child directory.
    #[must_use]
    pub fn from_directory_name(value: &str) -> Option<Self> {
        match value {
            LEGACY_PREPARATION_REFERENCES_DIRECTORY => Some(Self::References),
            LEGACY_PREPARATION_OBJECTS_DIRECTORY => Some(Self::Objects),
            _ => None,
        }
    }
}

/// Closed migration-inventory classification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyPreparationInventoryKind {
    /// Reference bytes are structurally valid but not yet matched to the current catalog/profile.
    UnmappedReference,
    /// Object has an admitted final name but is not linked by the current profile.
    UnmappedObject,
    /// Writer-generated temporary artifact with no committed final locator.
    Temporary,
    /// Structurally valid reference matched to one current catalog revision/profile.
    CurrentReference,
    /// Exact final object named by one current-profile reference.
    CurrentTarget,
}

impl LegacyPreparationInventoryKind {
    /// Stable migration-report tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::CurrentReference => "current_profile_reference",
            Self::UnmappedReference => {
                "unmapped_profile_or_revision_reference"
            }
            Self::CurrentTarget => "current_profile_target",
            Self::UnmappedObject => "not_linked_by_current_profile",
            Self::Temporary => "uncommitted_temporary_object",
        }
    }
}

/// One admitted basename classification with its lower-case digest identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyPreparationInventoryName<'a> {
    id: &'a str,
    kind: LegacyPreparationInventoryKind,
}

impl<'a> LegacyPreparationInventoryName<'a> {
    /// Lower-case 64-character digest identity from the basename.
    #[must_use]
    pub const fn id(self) -> &'a str {
        self.id
    }

    /// Initial physical classification before current-catalog overlay.
    #[must_use]
    pub const fn kind(self) -> LegacyPreparationInventoryKind {
        self.kind
    }
}

/// Classifies one exact basename in an admitted physical tree.
///
/// Unknown extensions, upper-case/non-hex IDs, nondecimal temporary fields,
/// extra components and overlong/non-ASCII names are rejected.
#[must_use]
pub fn classify_legacy_preparation_inventory_name(
    tree: LegacyPreparationInventoryTree,
    name: &str,
) -> Option<LegacyPreparationInventoryName<'_>> {
    if name.len() > LEGACY_PREPARATION_MAX_INVENTORY_NAME_BYTES
        || !name.is_ascii()
    {
        return None;
    }

    match tree {
        LegacyPreparationInventoryTree::References => {
            if let Some(id) = name
                .strip_suffix(".ref")
                .filter(|id| lower_hex_len(id, 64))
            {
                return Some(LegacyPreparationInventoryName {
                    id,
                    kind: LegacyPreparationInventoryKind::UnmappedReference,
                });
            }
        }
        LegacyPreparationInventoryTree::Objects => {
            for protection in [
                LegacyPreparationProtection::Plaintext,
                LegacyPreparationProtection::Protected,
            ] {
                let suffix = match protection {
                    LegacyPreparationProtection::Plaintext => ".bin",
                    LegacyPreparationProtection::Protected => ".dpapi",
                };
                if let Some(id) = name
                    .strip_suffix(suffix)
                    .filter(|id| lower_hex_len(id, 64))
                {
                    return Some(LegacyPreparationInventoryName {
                        id,
                        kind: LegacyPreparationInventoryKind::UnmappedObject,
                    });
                }
            }
        }
    }

    // The legacy immutable publisher uses the same exact temporary grammar in
    // both trees, including the historical `.dpapi.tmp` suffix for plaintext.
    let mut fields = name
        .strip_prefix('.')?
        .strip_suffix(".dpapi.tmp")?
        .split('.');
    let id = fields.next()?;
    let process = fields.next()?;
    let timestamp = fields.next()?;
    if fields.next().is_some()
        || !lower_hex_len(id, 64)
        || !decimal(process)
        || !decimal(timestamp)
    {
        return None;
    }
    Some(LegacyPreparationInventoryName {
        id,
        kind: LegacyPreparationInventoryKind::Temporary,
    })
}

/// Canonical relative locator for one lookup-reference key.
#[must_use]
pub fn legacy_preparation_reference_relative_locator(
    key: &[u8; 32],
) -> String {
    format!(
        "{}/{}/{}",
        LEGACY_PREPARATION_REFERENCES_DIRECTORY,
        legacy_preparation_shard(key),
        legacy_preparation_reference_file_name(key),
    )
}

/// Canonical relative locator for one encoded preparation object.
#[must_use]
pub fn legacy_preparation_object_relative_locator(
    id: &[u8; 32],
    protection: LegacyPreparationProtection,
) -> String {
    format!(
        "{}/{}/{}",
        LEGACY_PREPARATION_OBJECTS_DIRECTORY,
        legacy_preparation_shard(id),
        legacy_preparation_object_file_name(id, protection),
    )
}

/// Canonical data-root-relative locator for a validated preparation-tree item.
#[must_use]
pub fn legacy_preparation_rooted_locator(relative: &str) -> String {
    format!("{LEGACY_PREPARATION_DIRECTORY}/{relative}")
}

fn lower_hex_len(value: &str, length: usize) -> bool {
    value.len() == length
        && value.bytes().all(|byte| {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        })
}

fn decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests;
