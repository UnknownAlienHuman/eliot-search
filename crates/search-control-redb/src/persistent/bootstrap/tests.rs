//! Real journal/artifact I/O with synthetic native identities and digest inputs.
//! These are causal owner cases, not native/BLAKE3/installed qualification.
use super::*;
use crate::migration::{SourceImportOutputArtifactPlatform, SourceImportOutputLockPlatform,
    SourceImportRecordArtifact};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::Duration;

pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("eliot-bootstrap-{}-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn file(&self) -> File {
        std::fs::OpenOptions::new().read(true).write(true).create_new(true)
            .open(self.0.join("target.redb")).unwrap()
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

pub(super) fn deadline() -> Instant { Instant::now() + Duration::from_secs(30) }
// Injected deterministic fixture function, deliberately not a crypto algorithm.
// Production invokes the composition-owned real BLAKE3 primitive instead.
pub(super) fn fixture_digest(bytes: &[u8]) -> Blake3Digest32 {
    let mut output = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        let slot = index % 32;
        output[slot] = output[slot].wrapping_mul(17).wrapping_add(*byte).wrapping_add(u8::try_from(slot).expect("slot is less than 32"));
    }
    Blake3Digest32::from_bytes(output)
}
pub(super) fn identity(file: u64) -> MigrationBootstrapIdentity {
    MigrationBootstrapIdentity { profile: MigrationBootstrapIdentityProfile::WindowsFileIndex64,
        volume: 1, file }
}
pub(super) fn binding() -> MigrationBootstrapBinding {
    MigrationBootstrapBinding {
        owner: LegacyInventoryOwner { installation: [1;16], incarnation: [2;16], root: [3;16],
            invocation: [4;16], request_blake3: [5;32], record_blake3: [6;32], physical_blake3: [7;32],
            native_limits: [10,20,30,40,50,60], epoch: 1, generation: 2 },
        target: SourceNamespaceId::from_bytes([8;16]), legacy_namespace: Sha256Digest32::from_bytes([9;32]),
        catalog_snapshot: Sha256Digest32::from_bytes([10;32]), registration: None,
        output: MigrationBootstrapOutput { directory: identity(2), target: identity(3),
            path_blake3: Blake3Digest32::from_bytes([11;32]) },
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Platform {
    pub(super) cancel_after_sync: Arc<AtomicBool>,
    pub(super) canceled: Arc<AtomicBool>,
    pub(super) fail_sync: Arc<AtomicBool>,
}
impl SourceImportOutputLockPlatform for Platform {
    type Error = std::io::Error;
    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        if path.is_dir() { Ok(()) } else { Err(std::io::Error::other("fixture directory")) }
    }
    fn verify_locator(&self, _: &File, path: &Path) -> Result<(), Self::Error> {
        if path.is_file() { Ok(()) } else { Err(std::io::Error::other("fixture locator")) }
    }
    fn sync_directory(&self, _: &Path) -> Result<(), Self::Error> { Ok(()) }
}
impl SourceImportOutputArtifactPlatform for Platform {
    type Identity = u64;
    fn identity(&self, _: &File) -> Result<u64, Self::Error> { Ok(1) }
}
impl SourceImportRecordArtifactPlatform for Platform {
    fn sync_record_artifact_directory(&self, _: &Path) -> Result<(), Self::Error> {
        if self.cancel_after_sync.load(Ordering::Relaxed) { self.canceled.store(true, Ordering::Relaxed); }
        if self.fail_sync.load(Ordering::Relaxed) { Err(std::io::Error::other("fixture sync")) } else { Ok(()) }
    }
}
impl MigrationBootstrapPlatform for Platform {
    fn bootstrap_identity(&self, value: &u64) -> MigrationBootstrapIdentity { identity(*value) }
}
pub(super) fn artifact(scratch: &Scratch, platform: Platform) -> SourceImportRecordArtifactPublication<Platform> {
    let deadline = deadline();
    let mut writer = SourceImportRecordArtifact::create(&scratch.0, ".source-map.fixture.tmp", platform, deadline).unwrap();
    writer.push(b"fixture metadata row\n", deadline).unwrap();
    let frozen = writer.freeze(deadline).unwrap();
    let name = format!("{}.legacy-inventory.v1", codec::hex(frozen.chain()));
    let mut reader = frozen.begin_readback(deadline).unwrap();
    reader.compare(b"fixture metadata row\n", deadline).unwrap();
    reader.finish(deadline).unwrap().prepare_publication(&name, deadline).unwrap()
}

#[test]
fn original_context_refusal_prevents_journal_initialization_and_retains_file() {
    let scratch = Scratch::new();
    let file = scratch.file();
    let retained = file.try_clone().unwrap();
    let result = MigrationBootstrap::create(file, binding(), deadline(), fixture_digest,
        |_| Err::<(), _>("private original refusal"));
    let failure = result.err().unwrap();
    assert_eq!(failure.control, ControlError::ReadCancelled);
    assert_eq!(failure.first_context, Some("private original refusal"));
    assert!(!format!("{failure:?}").contains("private original refusal"));
    assert_eq!(retained.metadata().unwrap().len(), 0);
}

#[test]
fn exact_original_binding_and_work_are_durable_before_artifact_dispatch() {
    let scratch = Scratch::new();
    let work = RefCell::new(Vec::new());
    let mut journal = MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest,
        |value| { work.borrow_mut().push(value); Ok::<(), ()>(()) }).unwrap();
    let snapshot = journal.journal.read_snapshot().unwrap();
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].1.class(), ControlRecordClass::Migration);
    assert_eq!(snapshot.records[0].1.as_bytes(), codec::original(&binding()).unwrap());
    assert!(work.borrow().contains(&MigrationBootstrapWork::Allocate(super::super::CACHE_BYTES)));
    assert!(work.borrow().iter().any(|work| matches!(work, MigrationBootstrapWork::Write(_))));
    let platform = Platform::default();
    let mut publication = artifact(&scratch, platform.clone());
    let receipt = journal.publish_inventory(&mut publication, &platform, deadline(), |_| Ok::<(), ()>(())).unwrap();
    assert!(scratch.0.join(publication.final_name()).is_file());
    let json = receipt.render_json();
    assert!(json.contains("complete_candidate_not_performed"));
    assert!(json.contains(&MigrationBootstrap::target_name(&binding().owner)));
    assert_eq!(journal.journal.read_snapshot().unwrap().records.len(), 2);
}

#[test]
fn refused_pre_dispatch_phase_cannot_create_final_artifact() {
    let scratch = Scratch::new();
    let mut journal = MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest,
        |_| Ok::<(), ()>(())).unwrap();
    let platform = Platform::default();
    let mut publication = artifact(&scratch, platform.clone());
    let failure = journal.publish_inventory(&mut publication, &platform, deadline(),
        |_| Err::<(), _>("phase refused")).err().unwrap();
    assert_eq!(failure.first_context, Some("phase refused"));
    assert!(!scratch.0.join(publication.final_name()).exists());
    assert!(scratch.0.join(".source-map.fixture.tmp").exists());
    assert_eq!(journal.journal.read_snapshot().unwrap().records.len(), 1);
}

#[test]
fn post_link_refusal_keeps_dispatch_phase_and_blocks_replay() {
    let scratch = Scratch::new();
    let mut journal = MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest,
        |_| Ok::<(), ()>(())).unwrap();
    let platform = Platform::default();
    platform.cancel_after_sync.store(true, Ordering::Relaxed);
    let mut publication = artifact(&scratch, platform.clone());
    let failure = journal.publish_inventory(&mut publication, &platform, deadline(), |_| {
        if platform.canceled.load(Ordering::Relaxed) { Err("original canceled after link") } else { Ok(()) }
    }).err().unwrap();
    assert_eq!(failure.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(failure.first_context, Some("original canceled after link"));
    assert!(scratch.0.join(publication.final_name()).is_file());
    assert!(scratch.0.join(".source-map.fixture.tmp").is_file());
    let original = fixture_digest(&codec::framed(b"original", &codec::original(&binding()).unwrap()));
    let snapshot = journal.journal.read_snapshot().unwrap();
    let attempt = snapshot.get(&ControlKey::new(ATTEMPT_KEY.to_vec(), LIMITS).unwrap()).unwrap();
    assert_eq!(attempt.as_bytes(), codec::attempt(original, &publication, identity(1), 0).unwrap());
    assert_eq!(journal.publish_inventory(&mut publication, &platform, deadline(), |_| Ok::<(), ()>(()))
        .err().unwrap().control, ControlError::CommitOutcomeUnknown);
    assert_eq!(journal.journal.read_snapshot().unwrap(), snapshot);
}

#[test]
fn metadata_profile_binds_actual_limits_invocation_and_native_width() {
    let original = codec::original(&binding()).unwrap();
    let mut changed = binding(); changed.owner.native_limits[5] += 1;
    assert_ne!(codec::original(&changed).unwrap(), original);
    changed = binding(); changed.owner.invocation[0] += 1;
    assert_ne!(codec::original(&changed).unwrap(), original);
    changed = binding(); changed.output.target.profile = MigrationBootstrapIdentityProfile::UnixInode64;
    assert_ne!(codec::original(&changed).unwrap(), original);
    changed = binding(); changed.output.target.volume = u64::from(u32::MAX) + 1;
    assert_eq!(codec::original(&changed), Err(ControlError::IdentityMismatch));
    assert!(!original.windows(b"ACTIVE".len()).any(|part| part == b"ACTIVE"));
}
