//! Pure identity binding for the persistent technical control journal.
//!
//! Native path observation and BLAKE3 hashing remain platform-composition
//! responsibilities. This module accepts only already verified fixed-size path
//! and schema digests, then binds them to the exact live owner coordinates. A
//! path string, database header or caller-selected owner epoch cannot substitute
//! for those independently produced observations.

use search_contracts::{
    Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch,
};

use crate::{ControlError, JournalIdentity};

/// Current concrete redb schema version owned by this package.
pub const CURRENT_JOURNAL_SCHEMA_VERSION: u32 = 1;

const CURRENT_JOURNAL_SCHEMA_MATERIAL: &[u8] = b"ELIOT-CONTROL-REDB-SCHEMA-FAMILY-v1\0header=ELCTRL01\0receipt=ELCTOP01\0table=eliot.control.meta.v1\0table=eliot.control.records.v1\0table=eliot.control.operations.v1\0record-classes=identity,revision,state,receipt,operation,snapshot,migration\0";

/// Exact root-owner coordinates used to bind one journal identity.
///
/// Construction does not create an owner or prove that its OS lock is held. The
/// daemon must retain the corresponding non-clonable owner guard for the entire
/// journal lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalOwnerBinding {
    installation_incarnation_id: InstallationIncarnationId,
    data_root_id: DataRootId,
    owner_epoch: OwnerEpoch,
}

impl JournalOwnerBinding {
    /// Retains independently verified live-owner coordinates.
    #[must_use]
    pub const fn new(
        installation_incarnation_id: InstallationIncarnationId,
        data_root_id: DataRootId,
        owner_epoch: OwnerEpoch,
    ) -> Self {
        Self {
            installation_incarnation_id,
            data_root_id,
            owner_epoch,
        }
    }

    /// Installation incarnation owning the data root.
    #[must_use]
    pub const fn installation_incarnation_id(self) -> InstallationIncarnationId {
        self.installation_incarnation_id
    }

    /// Exact physical data-root identity.
    #[must_use]
    pub const fn data_root_id(self) -> DataRootId {
        self.data_root_id
    }

    /// Owner epoch stored in, or about to own, the journal.
    #[must_use]
    pub const fn owner_epoch(self) -> OwnerEpoch {
        self.owner_epoch
    }
}

/// BLAKE3 digest of one exact canonical journal locator plus final opened object.
///
/// The platform adapter must bind both the canonical locator under the owned
/// root and the final regular-file native identity before constructing this
/// value. A digest of path text alone is insufficient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalPathIdentity {
    digest: Blake3Digest32,
}

impl JournalPathIdentity {
    /// Accepts one independently derived nonzero BLAKE3 path/object digest.
    pub fn new(digest: Blake3Digest32) -> Result<Self, ControlError> {
        if digest.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ControlError::IdentityMismatch);
        }
        Ok(Self { digest })
    }

    /// Exact path/object identity digest.
    #[must_use]
    pub const fn digest(self) -> Blake3Digest32 {
        self.digest
    }
}

/// Exact package schema family digest and nonzero schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalSchemaIdentity {
    family_digest: Blake3Digest32,
    version: u32,
}

impl JournalSchemaIdentity {
    /// Accepts one package-owned nonzero BLAKE3 schema digest and version.
    pub fn new(
        family_digest: Blake3Digest32,
        version: u32,
    ) -> Result<Self, ControlError> {
        if version == 0 || family_digest.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ControlError::SchemaUnsupported);
        }
        Ok(Self {
            family_digest,
            version,
        })
    }

    /// Exact schema family digest.
    #[must_use]
    pub const fn family_digest(self) -> Blake3Digest32 {
        self.family_digest
    }

    /// Exact schema version.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }
}

/// Canonical bounded material whose BLAKE3 digest identifies the current schema.
///
/// This descriptor contains no path, root, journal contents or authority. The
/// caller hashes these exact bytes with real BLAKE3 and passes the result through
/// [`JournalSchemaIdentity::new`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalSchemaDescriptor {
    material: &'static [u8],
    version: u32,
}

impl JournalSchemaDescriptor {
    /// Exact domain-separated schema material.
    #[must_use]
    pub const fn material(self) -> &'static [u8] {
        self.material
    }

    /// Exact schema version represented by the material.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }
}

/// Returns the sole schema descriptor accepted by the current persistent adapter.
#[must_use]
pub const fn current_journal_schema_descriptor() -> JournalSchemaDescriptor {
    JournalSchemaDescriptor {
        material: CURRENT_JOURNAL_SCHEMA_MATERIAL,
        version: CURRENT_JOURNAL_SCHEMA_VERSION,
    }
}

/// Binds verified owner, path/object and schema identities into one journal key.
///
/// This operation performs no filesystem, redb or owner-lock I/O. The supplied
/// path and schema digests must already have been generated by their respective
/// platform/package owners; the database header is not an authority source for
/// any expected field.
pub fn derive_journal_identity(
    owner: JournalOwnerBinding,
    path: JournalPathIdentity,
    schema: JournalSchemaIdentity,
) -> Result<JournalIdentity, ControlError> {
    JournalIdentity {
        installation_incarnation_id: owner.installation_incarnation_id,
        data_root_id: owner.data_root_id,
        owner_epoch: owner.owner_epoch,
        path_identity_digest: path.digest,
        schema_family_digest: schema.family_digest,
        schema_version: schema.version,
    }
    .validate()
}
