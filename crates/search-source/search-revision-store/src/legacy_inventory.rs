//! Pure layout and physical-inventory grammar for legacy DIRECT revisions.
//!
//! Filesystem traversal, metadata observation, source-catalog membership and
//! content hashing remain daemon composition. This owner closes the legacy
//! revision root, object-size ceiling, final/temporary basename grammar,
//! classification tags and canonical relative locators used during migration.

#![allow(
    clippy::missing_const_for_fn,
    clippy::module_name_repetitions,
)]

/// Canonical legacy revision-object directory below the DIRECT data root.
pub const LEGACY_REVISION_DIRECTORY: &str = "revisions";
/// Maximum encoded bytes admitted for one legacy revision object.
pub const LEGACY_REVISION_MAX_OBJECT_BYTES: usize = 65 * 1024 * 1024;
/// Maximum one-file basename admitted by the legacy revision inventory.
pub const LEGACY_REVISION_MAX_INVENTORY_NAME_BYTES: usize = 192;

/// Closed persisted encoding suffix for a final legacy revision object.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionProtection {
    /// Historical plaintext object (`.bin`).
    Plaintext,
    /// Protected object (`.dpapi`).
    Protected,
}

impl LegacyRevisionProtection {
    /// Canonical final-file extension without a leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Plaintext => "bin",
            Self::Protected => "dpapi",
        }
    }
}

/// Physical basename class before source-catalog overlay.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionPhysicalKind {
    /// Final immutable object with its persisted protection class.
    Final(LegacyRevisionProtection),
    /// Writer-generated, uncommitted temporary object.
    Temporary,
}

/// Closed migration-inventory classification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyRevisionInventoryKind {
    /// Final object referenced by the admitted source catalog.
    Referenced,
    /// Final object not referenced by the admitted source catalog.
    Orphan,
    /// Writer-generated temporary artifact.
    Temporary,
}

impl LegacyRevisionInventoryKind {
    /// Stable migration-report tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Referenced => "catalog_referenced",
            Self::Orphan => "unreferenced_revision_object",
            Self::Temporary => "uncommitted_temporary_object",
        }
    }
}

/// One admitted basename classification with its lower-case revision identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRevisionInventoryName<'a> {
    id: &'a str,
    physical_kind: LegacyRevisionPhysicalKind,
}

impl<'a> LegacyRevisionInventoryName<'a> {
    /// Lower-case 64-character revision identity from the basename.
    #[must_use]
    pub const fn id(self) -> &'a str {
        self.id
    }

    /// Physical basename class before catalog overlay.
    #[must_use]
    pub const fn physical_kind(self) -> LegacyRevisionPhysicalKind {
        self.physical_kind
    }

    /// Applies current source-catalog membership to the physical object class.
    #[must_use]
    pub const fn inventory_kind(
        self,
        catalog_referenced: bool,
    ) -> LegacyRevisionInventoryKind {
        match self.physical_kind {
            LegacyRevisionPhysicalKind::Temporary => {
                LegacyRevisionInventoryKind::Temporary
            }
            LegacyRevisionPhysicalKind::Final(_) if catalog_referenced => {
                LegacyRevisionInventoryKind::Referenced
            }
            LegacyRevisionPhysicalKind::Final(_) => {
                LegacyRevisionInventoryKind::Orphan
            }
        }
    }
}

/// Parses one exact lower-case two-character revision shard.
#[must_use]
pub fn is_legacy_revision_inventory_shard(value: &str) -> bool {
    lower_hex_len(value, 2)
}

/// Classifies one exact basename from the legacy revision tree.
///
/// Accepted final names are `<lower-hex-64>.bin` and
/// `<lower-hex-64>.dpapi`. Accepted historical/current temporary names are
/// `.<lower-hex-64>.<decimal-pid>.tmp` and
/// `.<lower-hex-64>.<decimal-pid>.<decimal-time>.dpapi.tmp`.
/// Unknown extensions, upper-case/nonhex IDs, extra components and
/// overlong/non-ASCII names are rejected.
#[must_use]
pub fn classify_legacy_revision_inventory_name(
    name: &str,
) -> Option<LegacyRevisionInventoryName<'_>> {
    if name.len() > LEGACY_REVISION_MAX_INVENTORY_NAME_BYTES
        || !name.is_ascii()
    {
        return None;
    }

    for protection in [
        LegacyRevisionProtection::Plaintext,
        LegacyRevisionProtection::Protected,
    ] {
        let suffix = match protection {
            LegacyRevisionProtection::Plaintext => ".bin",
            LegacyRevisionProtection::Protected => ".dpapi",
        };
        if let Some(id) = name
            .strip_suffix(suffix)
            .filter(|id| lower_hex_len(id, 64))
        {
            return Some(LegacyRevisionInventoryName {
                id,
                physical_kind: LegacyRevisionPhysicalKind::Final(protection),
            });
        }
    }

    let body = name.strip_prefix('.')?.strip_suffix(".tmp")?;
    let mut fields = body.split('.');
    let id = fields.next()?;
    let process = fields.next()?;
    let timestamp = fields.next();
    let protection = fields.next();
    if fields.next().is_some()
        || !lower_hex_len(id, 64)
        || !decimal(process)
        || !matches!(
            (timestamp, protection),
            (None, None) | (Some(_), Some("dpapi"))
        )
        || timestamp.is_some_and(|value| !decimal(value))
    {
        return None;
    }
    Some(LegacyRevisionInventoryName {
        id,
        physical_kind: LegacyRevisionPhysicalKind::Temporary,
    })
}

/// Canonical final basename for one validated revision identity.
#[must_use]
pub fn legacy_revision_object_file_name(
    revision_id: &str,
    protection: LegacyRevisionProtection,
) -> Option<String> {
    lower_hex_len(revision_id, 64).then(|| {
        let extension = protection.extension();
        format!("{revision_id}.{extension}")
    })
}

/// Canonical shard-relative locator for one final revision object.
#[must_use]
pub fn legacy_revision_object_relative_locator(
    revision_id: &str,
    protection: LegacyRevisionProtection,
) -> Option<String> {
    let name = legacy_revision_object_file_name(revision_id, protection)?;
    let shard = &revision_id[..2];
    Some(format!("{shard}/{name}"))
}

/// Canonical relative locator for one already observed inventory basename.
///
/// The shard, basename grammar and identity/shard correspondence are all
/// validated before a locator is returned.
#[must_use]
pub fn legacy_revision_inventory_relative_locator(
    shard: &str,
    name: &str,
) -> Option<String> {
    if !is_legacy_revision_inventory_shard(shard) {
        return None;
    }
    let classified = classify_legacy_revision_inventory_name(name)?;
    classified
        .id()
        .starts_with(shard)
        .then(|| format!("{shard}/{name}"))
}

/// Canonical data-root-relative locator for a validated inventory item.
#[must_use]
pub fn legacy_revision_rooted_locator(relative: &str) -> Option<String> {
    let (shard, name) = relative.split_once('/')?;
    if name.contains('/')
        || legacy_revision_inventory_relative_locator(shard, name).as_deref()
            != Some(relative)
    {
        return None;
    }
    Some(format!("{LEGACY_REVISION_DIRECTORY}/{relative}"))
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
