//! Native journal identity composition shared by default DIRECT and provider wiring.
use eliot_searchd::native_file::Observation;
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use search_control_redb::{
    ControlError, JournalIdentity, JournalOwnerBinding, JournalPathIdentity, JournalSchemaIdentity,
    current_journal_schema_descriptor, derive_journal_identity,
};

const CONTROL_RELATIVE_LOCATOR: &[u8] = b"control/control.redb";
const LOCATOR_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-CANONICAL-LOCATOR-v1\0";
const OBJECT_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-NATIVE-OBJECT-v1\0";
const PATH_IDENTITY_DOMAIN: &[u8] = b"ELIOT-CONTROL-JOURNAL-PATH-IDENTITY-v1\0";

/// Derive the existing identity recipe from independently verified Native values.
///
/// # Errors
/// Returns an error when the owner, path or schema identity is invalid.
pub fn journal_identity_from_observation(
    incarnation: InstallationIncarnationId,
    data_root_id: DataRootId,
    stored_owner_epoch: OwnerEpoch,
    observation: Observation,
) -> Result<JournalIdentity, ControlError> {
    let locator_digest = canonical_locator_digest(data_root_id);
    let object_digest = native_object_digest(observation);
    let path_digest = combined_path_digest(locator_digest, object_digest);
    let path_identity = JournalPathIdentity::new(path_digest)?;
    let schema = current_journal_schema_descriptor();
    let schema_digest = Blake3Digest32::from_bytes(*blake3::hash(schema.material()).as_bytes());
    let schema_identity = JournalSchemaIdentity::new(schema_digest, schema.version())?;
    let owner = JournalOwnerBinding::new(incarnation, data_root_id, stored_owner_epoch);
    derive_journal_identity(owner, path_identity, schema_identity)
}

fn canonical_locator_digest(data_root_id: DataRootId) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(LOCATOR_DOMAIN);
    hash.update(data_root_id.as_bytes());
    hash.update(CONTROL_RELATIVE_LOCATOR);
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

fn native_object_digest(observation: Observation) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(OBJECT_DOMAIN);
    hash.update(&observation.volume_serial.to_be_bytes());
    hash.update(&observation.file_index.to_be_bytes());
    hash.update(&observation.creation_time.to_be_bytes());
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

fn combined_path_digest(locator: Blake3Digest32, object: Blake3Digest32) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(PATH_IDENTITY_DOMAIN);
    hash.update(locator.as_bytes());
    hash.update(object.as_bytes());
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}
