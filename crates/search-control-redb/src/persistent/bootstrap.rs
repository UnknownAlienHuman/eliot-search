//! Original migration metadata in the single future inactive target.
//! No normal journal, serving snapshot, candidate, retry or cleanup capability escapes.

use std::cell::RefCell;
use std::fs::File;
use std::time::Instant;

use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch,
    Sha256Digest32, SourceNamespaceId};
use crate::migration::{LegacyInventoryOwner, SourceImportRecordArtifactPlatform,
    SourceImportRecordArtifactPublication, SourceImportPublishedRecordArtifact};
use crate::{ConditionalControlMutation, ControlError, ControlKey, ControlMutation,
    ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite, JournalIdentity,
    JournalLimits, MutationId};
use super::operation::{Check, Point};
use super::{Boundary, PersistentControlJournal};

mod codec;
#[path = "source_stage.rs"]
mod source_stage;
pub use source_stage::{OwnerBoundSourceStage, OwnerSourceMappingReadback, OwnerSourceMappedReceipt};
pub use source_stage::{OwnerInventoryStage, OwnerInventoryReadback, OwnerInventoryMappedReceipt};

const LIMITS: JournalLimits = JournalLimits {
    max_key_bytes: 128, max_value_bytes: 4096, max_records: 8,
    max_total_value_bytes: 32 * 1024, max_mutation_items: 3, max_operation_records: 8,
};
const OPERATION_KEY: &[u8] = b"migration/bootstrap/v1/original";
const ATTEMPT_KEY: &[u8] = b"migration/bootstrap/v1/inventory-attempt";

/// Explicit width/platform profile of an original opened object observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MigrationBootstrapIdentityProfile {
    /// Windows volume serial (32 bits) and `FILE_INDEX` (64 bits), not `FILE_ID_128`.
    WindowsFileIndex64 = 1,
    /// Unix device and inode (each 64 bits).
    UnixInode64 = 2,
}

/// Content-free stable observation from an original opened native object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationBootstrapIdentity {
    /// Exact admitted native representation; widths are never silently reinterpreted.
    pub profile: MigrationBootstrapIdentityProfile,
    /// Observed volume serial or device ID.
    pub volume: u64,
    /// Observed file index or inode.
    pub file: u64,
}

/// Retained external output directory and newly created future target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationBootstrapOutput {
    /// Original pinned directory identity.
    pub directory: MigrationBootstrapIdentity,
    /// Original opened target identity, captured before journal initialization.
    pub target: MigrationBootstrapIdentity,
    /// Identity digest of the exact canonical target locator; no path bytes persist.
    pub path_blake3: Blake3Digest32,
}

/// Metadata supplied under the actual retained read child, never ACTIVE custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationBootstrapBinding {
    /// Original child request/root/owner observation and admitted limits.
    pub owner: LegacyInventoryOwner,
    /// Explicit future import target namespace, not source authority.
    pub target: SourceNamespaceId,
    /// Exact retained legacy namespace observation.
    pub legacy_namespace: Sha256Digest32,
    /// Exact original source-event snapshot.
    pub catalog_snapshot: Sha256Digest32,
    /// Original retained registration bytes, represented only by digest and length.
    pub registration: Option<(Sha256Digest32, u64)>,
    /// Original external output and target observations.
    pub output: MigrationBootstrapOutput,
}

/// Narrow native serialization adapter; invoked on the retained original identity.
pub trait MigrationBootstrapPlatform: SourceImportRecordArtifactPlatform {
    /// Preserves the admitted native width/profile without observing a current path.
    fn bootstrap_identity(&self, original: &Self::Identity) -> MigrationBootstrapIdentity;
}

/// Finite logical work reserved against the original borrowed native allowance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationBootstrapWork {
    /// Poll the original context without replacing its budget or deadline.
    Checkpoint,
    /// One native store-open dispatch.
    Open,
    /// Logical buffer/cache capacity before allocation.
    Allocate(usize),
    /// Bounded metadata read/compare capacity before observation.
    Read(usize),
    /// Bounded in-memory metadata copying, comparison or hashing work.
    Process(usize),
    /// Bounded metadata staging or durability dispatch.
    Write(usize),
}

/// A failed concrete call and its first original-context refusal, if any.
/// Debug never formats the platform/context cause.
pub struct MigrationBootstrapFailure<E> {
    /// Owning control result, including possible mutation outcome.
    pub control: ControlError,
    /// First refusal from the original borrowed context, preserved without renewal.
    pub first_context: Option<E>,
}
impl<E> std::fmt::Debug for MigrationBootstrapFailure<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationBootstrapFailure").field("control", &self.control)
            .field("context_refused", &self.first_context.is_some()).finish()
    }
}

/// Opaque original-operation owner over one inactive, already opened target.
/// Drop closes handles only. It never deletes the target or retained artifact.
pub struct MigrationBootstrap {
    journal: PersistentControlJournal,
    original: Vec<u8>,
    digest: fn(&[u8]) -> Blake3Digest32,
    target_name: String,
    blocked: bool,
    binding: MigrationBootstrapBinding,
    completed_inventory: Option<CompletedInventory>,
}

// Minted only after the original artifact outcome's exact durable readback.
struct CompletedInventory {
    profile: crate::migration::LegacyInventoryProfile,
    original: Blake3Digest32,
    attempt: Vec<u8>,
    chain: [u8; 32],
    rows: u64,
    bytes: u64,
}

impl MigrationBootstrap {
    /// Deterministic local target basename for the actual original invocation.
    #[must_use]
    pub fn target_name(owner: &LegacyInventoryOwner) -> String {
        format!("{}.owner-migration.v1.redb", codec::hex(&owner.invocation))
    }

    /// Initializes a caller-created exclusive empty target under retained native pins.
    /// The caller retains the actual source read guard and output pins throughout.
    /// No existing target is reopened or treated as original-operation proof.
    /// # Errors
    /// Initialization/commit uncertainty retains the target and forbids dispatch.
    pub fn create<F, E>(file: File, binding: MigrationBootstrapBinding,
        deadline: Instant, digest: fn(&[u8]) -> Blake3Digest32, checkpoint: F)
        -> Result<Self, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            check.check(Point::Start)?;
            check.reserve(MigrationBootstrapWork::Allocate(4096))?;
            let original = codec::original(&binding)?;
            let identity = JournalIdentity {
                installation_incarnation_id: InstallationIncarnationId::from_bytes(binding.owner.incarnation),
                data_root_id: DataRootId::from_bytes(binding.owner.root),
                owner_epoch: OwnerEpoch::new(binding.owner.epoch).map_err(|_| ControlError::IdentityMismatch)?,
                path_identity_digest: binding.output.path_blake3,
                schema_family_digest: digest(&codec::framed(b"profile", codec::PROFILE)),
                schema_version: 1,
            };
            let journal = PersistentControlJournal::create_checked(file, identity, LIMITS, &check)?;
            let mut owner = Self { journal, original, digest,
                target_name: Self::target_name(&binding.owner), blocked: true,
                binding, completed_inventory: None };
            owner.write(OPERATION_KEY, owner.original.clone(), None, &[], &check)?;
            check.check(Point::LifecycleComplete)?;
            owner.blocked = false;
            Ok(owner)
        })();
        result.map_err(|control| check.failure(control))
    }

    /// Records the exact original inventory attempt durably before at most one link.
    /// A failed/unknown phase write cannot dispatch. After any possible effect the
    /// owner is permanently blocked unless the exact same-process outcome commits.
    /// # Errors
    /// Every post-dispatch refusal remains `CommitOutcomeUnknown`; restart/retry is
    /// deliberately unavailable. Equal current bytes cannot replace original identity.
    pub fn publish_inventory<P, F, E>(&mut self,
        publication: &mut SourceImportRecordArtifactPublication<P>, platform: &P,
        deadline: Instant, checkpoint: F,
    ) -> Result<MigrationBootstrapReceipt, MigrationBootstrapFailure<E>>
    where P: MigrationBootstrapPlatform, F: Fn(MigrationBootstrapWork) -> Result<(), E> {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let mut dispatched = false;
        let result = (|| {
            if self.blocked { return Err(ControlError::CommitOutcomeUnknown); }
            self.blocked = true;
            check.check(Point::Start)?;
            check.reserve(MigrationBootstrapWork::Allocate(8192))?;
            let original_identity = publication.original_identity()
                .map_err(|_| ControlError::MigrationUnverified)?;
            let identity = platform.bootstrap_identity(original_identity);
            let original_blake3 = (self.digest)(&codec::framed(b"original", &self.original));
            let profile = codec::inventory_profile(publication)?;
            let attempt = codec::attempt(original_blake3, publication, identity, 0)?;
            // This conditional immediate transaction performs exact post-state readback.
            // Its committed phase means "dispatch may occur", never success.
            self.write(ATTEMPT_KEY, attempt.clone(), None, OPERATION_KEY, &check)?;
            check.check(Point::BeforeWrite)?;
            dispatched = true;
            let published = publication
                .publish_once_with_context(deadline, &|unit| check.reserve(unit))
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)?;
            // publish_once syncs/readbacks the same original file identity, rows,
            // chain and EOF. A reused/foreign object cannot mint this outcome.
            if published.reused() { return Err(ControlError::CommitOutcomeUnknown); }
            let completed = codec::attempt(original_blake3, publication, identity, 1)?;
            self.write(ATTEMPT_KEY, completed.clone(), Some(attempt), OPERATION_KEY, &check)?;
            check.check(Point::LifecycleComplete)?;
            self.completed_inventory = Some(CompletedInventory {
                profile,
                original: original_blake3, attempt: completed,
                chain: *publication.chain(), rows: publication.rows(), bytes: publication.encoded_bytes(),
            });
            Ok(MigrationBootstrapReceipt {
                original_blake3: *original_blake3.as_bytes(),
                target_name: self.target_name.clone(),
                artifact: published,
                rows: publication.rows(),
            })
        })();
        result.map_err(|control| check.failure(if dispatched { ControlError::CommitOutcomeUnknown } else { control }))
    }

    fn write(&mut self, key_bytes: &[u8], bytes: Vec<u8>, previous: Option<Vec<u8>>,
        required_original: &[u8], check: &dyn Check,
    ) -> Result<(), ControlError> {
        let expected = self.original.clone();
        self.write_guarded(key_bytes, bytes, previous, required_original, expected, check)
    }

    fn write_guarded(&mut self, key_bytes: &[u8], bytes: Vec<u8>, previous: Option<Vec<u8>>,
        required_key: &[u8], required_value: Vec<u8>, check: &dyn Check,
    ) -> Result<(), ControlError> {
        check.check(Point::Start)?;
        let key = ControlKey::new(key_bytes.to_vec(), LIMITS)?;
        let value = ControlValue::new(ControlRecordClass::Migration, bytes, LIMITS)?;
        let mut conditions = vec![match previous {
            Some(bytes) => ControlRecordCondition::exact(key.clone(),
                ControlValue::new(ControlRecordClass::Migration, bytes, LIMITS)?),
            None => ControlRecordCondition::absent(key.clone()),
        }];
        if !required_key.is_empty() {
            conditions.push(ControlRecordCondition::exact(
                ControlKey::new(required_key.to_vec(), LIMITS)?,
                ControlValue::new(ControlRecordClass::Migration, required_value, LIMITS)?,
            ));
        }
        let snapshot = self.journal.read_snapshot_checked(check)?;
        let digest = (self.digest)(&codec::framed(key_bytes, value.as_bytes()));
        let mutation = ControlMutation::new(MutationId(*digest.as_bytes()), digest,
            snapshot.generation, vec![ControlWrite { key, value }], Vec::new());
        self.journal.transact_conditionally_checked(
            ConditionalControlMutation::new(mutation, conditions), Boundary::Normal, check)?;
        Ok(())
    }
}

/// Package-owned receipt of this bounded first phase, never a verified candidate.
/// Private fields cannot be populated from caller counts or current paths.
pub struct MigrationBootstrapReceipt {
    original_blake3: [u8; 32],
    target_name: String,
    artifact: SourceImportPublishedRecordArtifact,
    rows: u64,
}
impl MigrationBootstrapReceipt {
    /// Emits metadata only, explicitly retaining the missing full candidate phase.
    #[must_use]
    pub fn render_json(&self) -> String {
        format!("{{\"profile\":\"eliot.migration-bootstrap.v1\",\"target\":\"{}\",\"original_blake3\":\"{}\",\"inventory\":\"{}\",\"inventory_chain_sha256\":\"{}\",\"inventory_rows\":{},\"inventory_bytes\":{},\"gap\":\"complete_candidate_not_performed\"}}",
            self.target_name, codec::hex(&self.original_blake3), self.artifact.name(), codec::hex(self.artifact.chain()),
            self.rows, self.artifact.encoded_bytes())
    }
}

struct OriginalCheck<'a, F, E> {
    deadline: Instant,
    callback: &'a F,
    first: RefCell<Option<E>>,
}
impl<'a, F, E> OriginalCheck<'a, F, E> {
    const fn new(deadline: Instant, callback: &'a F) -> Self {
        Self { deadline, callback, first: RefCell::new(None) }
    }
    fn failure(self, control: ControlError) -> MigrationBootstrapFailure<E> {
        MigrationBootstrapFailure { control, first_context: self.first.into_inner() }
    }
}
impl<F: Fn(MigrationBootstrapWork) -> Result<(), E>, E> OriginalCheck<'_, F, E> {
    fn reserve(&self, work: MigrationBootstrapWork) -> Result<(), ControlError> {
        if self.first.borrow().is_some() { return Err(ControlError::ReadCancelled); }
        if let Err(error) = (self.callback)(work) {
            *self.first.borrow_mut() = Some(error);
            return Err(ControlError::ReadCancelled);
        }
        if Instant::now() >= self.deadline { return Err(ControlError::BudgetExceeded); }
        Ok(())
    }
}
impl<F: Fn(MigrationBootstrapWork) -> Result<(), E>, E> Check for OriginalCheck<'_, F, E> {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        use MigrationBootstrapWork as Work;
        let work = match point {
            Point::BeforeOpen => {
                self.reserve(Work::Allocate(super::CACHE_BYTES))?;
                Work::Open
            }
            Point::BeforeWrite | Point::BeforeCommit => Work::Write(0),
            Point::StageRecord => Work::Write(8192),
            Point::ReadHeader | Point::ReadRecord | Point::ReadOperation | Point::Readback => Work::Read(8192),
            _ => Work::Checkpoint,
        };
        self.reserve(work)
    }
}

#[cfg(test)]
mod tests;
