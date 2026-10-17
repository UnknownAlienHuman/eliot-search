//! Immutable daemon setup metadata in the existing C02 journal. A logical scope
//! is separate from the physical Root and grants no membership, access or profile.

use super::source_ids::map_source_identity_error;
use super::{RegistryJournal, SourceIdentityIdsV2, owner_failure, storage_failure};
use search_contracts::{self as c, CanonicalValue as V};
use search_control_redb::{
    self as db, ControlRecordClass as Class, ControlRecordCondition as Condition,
    MigrationBootstrapWork as Work,
};
use search_ports::{CancellationProbe, OperationContext};
use search_runtime_owner::OwnerError;
use search_source_registry::{
    api::SourceRegistry,
    error::{RegistryError, RegistryPortError},
    recovery::{
        RegistryStateKey, RegistryStateRecord, decode_registry_state_entry,
        decode_registry_state_key,
    },
    root::{RootRecord, RootStatus},
};

const PROFILE: &str = "eliot.daemon.root-source-scope.v1";
const PREFIX: &[u8] = b"eliot.daemon.root-source-scope.v1/";
const MAX_METADATA: usize = 16 * 1024;

/// Assigned logical IDs only. Repository lineage/workspace fingerprint and
/// selected member policies remain with their respective real owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootSourceScope {
    pub workspace_id: c::WorkspaceId,
    pub corpus_id: c::CorpusId,
}

impl<C: CancellationProbe> RegistryJournal<'_, C> {
    #[cfg(test)]
    pub(crate) fn scope_test_assignment(&mut self, registry: &SourceRegistry, root: &RootRecord,
        context: &OperationContext<C>, assigned: RootSourceScope,
    ) -> Result<RootSourceScope, RegistryPortError> {
        self.source_scope_with(registry, root, context, &mut || Ok(()), |_, _| Ok(assigned))
    }
    /// Select exactly one scope for this registered Root. The original Root
    /// registration operation, not a per-file operation, owns this setup.
    pub(crate) fn ensure_source_scope(
        &mut self,
        registry: &SourceRegistry,
        root: &RootRecord,
        context: &OperationContext<C>,
        current_root: &mut impl FnMut() -> Result<(), OwnerError>,
    ) -> Result<RootSourceScope, RegistryPortError> {
        self.source_scope_with(registry, root, context, current_root, |ids, context| {
            Ok(RootSourceScope {
                workspace_id: ids
                    .fresh_workspace_id(context)
                    .map_err(map_source_identity_error)?,
                corpus_id: ids
                    .fresh_corpus_id(context)
                    .map_err(map_source_identity_error)?,
            })
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep original lookup, bounded assignment and exact atomic recovery in one ordered setup"
    )]
    fn source_scope_with(
        &mut self,
        registry: &SourceRegistry,
        root: &RootRecord,
        context: &OperationContext<C>,
        current_root: &mut impl FnMut() -> Result<(), OwnerError>,
        mint: impl FnOnce(
            &mut SourceIdentityIdsV2<'_, C>,
            &OperationContext<C>,
        ) -> Result<RootSourceScope, RegistryPortError>,
    ) -> Result<RootSourceScope, RegistryPortError> {
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        if self.requires_recovery() {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        let original = self
            .original_v4
            .as_ref()
            .ok_or_else(|| owner_failure(RegistryError::AdmissionRestorationUnavailable))?;
        let (deadline, work) = (original.deadline, original.work);
        // Prepay bounded codec/key/condition copies; C02 meters its own I/O.
        for unit in [
            Work::Checkpoint,
            Work::Allocate(16 * MAX_METADATA),
            Work::Process(16 * MAX_METADATA),
        ] {
            work(unit).map_err(|error| {
                map_source_identity_error(super::SourceIdentityIdError::Owner(error))
            })?;
        }
        current_root().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        let (restored, operation) = self.restore_for_root(Some(root.root_binding_id), context)?;
        let operation =
            operation.ok_or_else(|| owner_failure(RegistryError::RootRestorationUnavailable))?;
        if root.status != RootStatus::Active
            || registry.roots().get(&root.root_binding_id) != Some(root)
            || restored.registry().revision() != registry.revision()
            || restored.registry().roots().get(&root.root_binding_id) != Some(root)
        {
            return Err(owner_failure(RegistryError::OperationConflict));
        }
        let snapshot = self
            .journal
            .read_registry_snapshot_with_original_context(deadline, &work)
            .map_err(|error| storage_failure(error.control, Some(&operation)))?;
        if snapshot.identity() != self.identity
            || snapshot.registry_revision() != registry.revision()
        {
            return Err(owner_failure(RegistryError::OperationConflict));
        }
        let state = snapshot
            .states()
            .iter()
            .find(|state| {
                state.key().family() == db::RegistryStateFamily::Root
                    && decode_registry_state_key(state.key().canonical_bytes())
                        == Ok(RegistryStateKey::Root(root.root_binding_id))
            })
            .ok_or_else(|| owner_failure(RegistryError::RootNotRegistered))?;
        if state.canonical_bytes().len() > MAX_METADATA
            || decode_registry_state_entry(state.key().canonical_bytes(), state.canonical_bytes())
                != Ok(RegistryStateRecord::Root(root.clone()))
        {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        let limits = self.journal.limits();
        let guard = state
            .key()
            .current_condition(state.canonical_bytes(), limits)
            .map_err(|error| storage_failure(error, Some(&operation)))?;
        let key = scope_key(b"root/", root.root_binding_id.as_bytes(), limits)?;
        let op_key = scope_key(b"operation/", operation.as_str().as_bytes(), limits)?;
        let keys = [key.clone(), op_key.clone(), guard.key().clone()];
        let read = self
            .journal
            .read_control_keys_with_original_context(&keys, deadline, &work)
            .map_err(|error| storage_failure(error.control, Some(&operation)))?;
        if read.identity() != self.identity
            || read.generation() != snapshot.journal_generation()
            || read.get(guard.key()) != guard.expected()
        {
            return Err(owner_failure(RegistryError::OperationConflict));
        }
        match (read.get(&key), read.get(&op_key)) {
            (Some(binding), Some(claim))
                if binding.class() == Class::State
                    && claim.class() == Class::Operation
                    && binding.as_bytes() == claim.as_bytes() =>
            {
                let stored = decode(
                    binding.as_bytes(),
                    root,
                    &operation,
                    registry.revision(),
                    read.generation(),
                )?;
                let initial_guard = state
                    .key()
                    .current_condition(&stored.root_bytes, limits)
                    .map_err(|error| storage_failure(error, Some(&operation)))?;
                let command = scope_command(
                    &keys,
                    initial_guard,
                    binding.as_bytes(),
                    stored.generation,
                    limits,
                )?;
                let identity = db::control_mutation_identity(command.mutation().id())
                    .map_err(|error| storage_failure(error, Some(&operation)))?;
                // Verify the original complete C02 fingerprint, including its
                // historical CAS. Never reinstall that historical Root state.
                self.original_uncertain = true;
                match self
                    .journal
                    .recover_records_with_original_context(&command, &identity, deadline, &work)
                {
                    Ok(db::CommitRecoveryDecision::Committed(_)) => self.original_uncertain = false,
                    _ => return Err(owner_failure(RegistryError::MutationOutcomeUnknown)),
                }
                current_root().map_err(|error| {
                    map_source_identity_error(super::SourceIdentityIdError::Owner(error))
                })?;
                context
                    .preflight()
                    .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
                return Ok(stored.assigned);
            }
            (None, None)
                if read.missing_keys().contains(&key) && read.missing_keys().contains(&op_key) => {}
            _ => return Err(owner_failure(RegistryError::OperationConflict)),
        }
        // All exact original/setup readbacks precede the first entropy draw.
        current_root().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        let assigned = mint(&mut SourceIdentityIdsV2::<C>::bind(deadline, work), context)?;
        let bytes = encode(
            &assigned,
            &operation,
            state.canonical_bytes(),
            registry.revision(),
            read.generation(),
        )?;
        let after_generation = read
            .generation()
            .checked_add(1)
            .ok_or_else(|| owner_failure(RegistryError::DurabilityExhausted))?;
        decode(
            &bytes,
            root,
            &operation,
            registry.revision(),
            after_generation,
        )?;
        let command = scope_command(&keys, guard.clone(), &bytes, read.generation(), limits)?;
        let identity = db::control_mutation_identity(command.mutation().id())
            .map_err(|error| storage_failure(error, Some(&operation)))?;
        let binding = db::ControlValue::new(Class::State, bytes.clone(), limits)
            .map_err(|error| storage_failure(error, Some(&operation)))?;
        let claim = db::ControlValue::new(Class::Operation, bytes, limits)
            .map_err(|error| storage_failure(error, Some(&operation)))?;
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        current_root().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        self.original_uncertain = true;
        if let Err(first) = self
            .journal
            .transact_records_with_original_context(&command, &identity, deadline, &work)
        {
            match self
                .journal
                .recover_records_with_original_context(&command, &identity, deadline, &work)
            {
                Ok(db::CommitRecoveryDecision::Committed(_)) => {}
                Ok(db::CommitRecoveryDecision::NotCommittedRetrySameOperation) => {
                    self.original_uncertain = false;
                    return Err(storage_failure(first.control, Some(&operation)));
                }
                _ => return Err(storage_failure(first.control, Some(&operation))),
            }
        }
        let observed = self
            .journal
            .read_control_keys_with_original_context(&keys, deadline, &work)
            .map_err(|error| storage_failure(error.control, Some(&operation)))?;
        if observed.identity() != self.identity
            || read.generation().checked_add(1) != Some(observed.generation())
            || observed.get(&key) != Some(&binding)
            || observed.get(&op_key) != Some(&claim)
            || observed.get(guard.key()) != guard.expected()
        {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        self.original_uncertain = false;
        current_root().map_err(|error| {
            map_source_identity_error(super::SourceIdentityIdError::Owner(error))
        })?;
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        Ok(assigned)
    }
}

fn scope_key(
    kind: &[u8],
    suffix: &[u8],
    limits: db::JournalLimits,
) -> Result<db::ControlKey, RegistryPortError> {
    db::ControlKey::new([PREFIX, kind, suffix].concat(), limits)
        .map_err(|error| storage_failure(error, None))
}
fn scope_command(
    keys: &[db::ControlKey; 3],
    guard: Condition,
    bytes: &[u8],
    generation: u64,
    limits: db::JournalLimits,
) -> Result<db::ConditionalControlMutation, RegistryPortError> {
    // Digest addresses the technical operation only, never a logical UUID.
    let mut address = blake3::Hasher::new();
    address.update(PREFIX);
    address.update(keys[1].as_bytes());
    let id = db::MutationId(*address.finalize().as_bytes());
    let digest = c::Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes());
    let value = |class| {
        db::ControlValue::new(class, bytes.to_vec(), limits)
            .map_err(|error| storage_failure(error, None))
    };
    Ok(db::ConditionalControlMutation::new(
        db::ControlMutation::new(
            id,
            digest,
            generation,
            vec![
                db::ControlWrite {
                    key: keys[0].clone(),
                    value: value(Class::State)?,
                },
                db::ControlWrite {
                    key: keys[1].clone(),
                    value: value(Class::Operation)?,
                },
            ],
            Vec::new(),
        ),
        vec![
            guard,
            Condition::absent(keys[0].clone()),
            Condition::absent(keys[1].clone()),
        ],
    ))
}
fn bytes(value: &[u8]) -> Result<V, RegistryPortError> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| owner_failure(RegistryError::SnapshotInvalid))
}
fn encode(
    scope: &RootSourceScope,
    operation: &c::OpaqueId,
    root: &[u8],
    revision: u64,
    generation: u64,
) -> Result<Vec<u8>, RegistryPortError> {
    let text = |value: &str| {
        c::CanonicalText::new(value)
            .map(V::Text)
            .map_err(|_| owner_failure(RegistryError::SnapshotInvalid))
    };
    let value = c::BoundedList::new(vec![
        text(PROFILE)?,
        text(operation.as_str())?,
        bytes(root)?,
        V::U64(revision),
        V::U64(generation),
        bytes(scope.workspace_id.as_bytes())?,
        bytes(scope.corpus_id.as_bytes())?,
    ])
    .map(V::Array)
    .map_err(|_| owner_failure(RegistryError::SnapshotInvalid))?;
    let encoded = c::to_canonical_cbor(&value)
        .map_err(|_| owner_failure(RegistryError::SnapshotInvalid))?
        .into_vec();
    if encoded.len() > MAX_METADATA {
        return Err(owner_failure(RegistryError::DurabilityExhausted));
    }
    Ok(encoded)
}
struct StoredScope {
    assigned: RootSourceScope,
    root_bytes: Vec<u8>,
    generation: u64,
}
fn decode(
    encoded: &[u8],
    root: &RootRecord,
    operation: &c::OpaqueId,
    revision: u64,
    generation: u64,
) -> Result<StoredScope, RegistryPortError> {
    let invalid = || owner_failure(RegistryError::SnapshotInvalid);
    if encoded.len() > MAX_METADATA {
        return Err(invalid());
    }
    let V::Array(fields) = c::parse_canonical_cbor(encoded).map_err(|_| invalid())? else {
        return Err(invalid());
    };
    let [
        V::Text(profile),
        V::Text(original),
        V::Bytes(root_bytes),
        V::U64(before_revision),
        V::U64(before_generation),
        V::Bytes(workspace),
        V::Bytes(corpus),
    ] = fields.as_slice()
    else {
        return Err(invalid());
    };
    if profile.as_str() != PROFILE
        || original.as_str() != operation.as_str()
        || *before_revision == 0
        || *before_revision > revision
        || *before_generation >= generation
    {
        return Err(invalid());
    }
    let RegistryStateRecord::Root(initial) =
        search_source_registry::recovery::decode_registry_state(root_bytes.as_slice())
            .map_err(|_| invalid())?
    else {
        return Err(invalid());
    };
    if initial.root_binding_id != root.root_binding_id
        || initial.canonical_root_digest != root.canonical_root_digest
        || initial.status != RootStatus::Active
        || initial.registry_revision > *before_revision
        || initial.record_revision > root.record_revision
    {
        return Err(invalid());
    }
    let uuid = |value: &[u8]| -> Result<[u8; 16], RegistryPortError> {
        let id: [u8; 16] = value.try_into().map_err(|_| invalid())?;
        if id[6] & 0xf0 != 0x40 || id[8] & 0xc0 != 0x80 {
            return Err(invalid());
        }
        Ok(id)
    };
    let workspace = uuid(workspace.as_slice())?;
    let corpus = uuid(corpus.as_slice())?;
    if workspace == corpus
        || &workspace == root.root_binding_id.as_bytes()
        || &corpus == root.root_binding_id.as_bytes()
    {
        return Err(invalid());
    }
    Ok(StoredScope {
        assigned: RootSourceScope {
            workspace_id: c::WorkspaceId::from_bytes(workspace),
            corpus_id: c::CorpusId::from_bytes(corpus),
        },
        root_bytes: root_bytes.as_slice().to_vec(),
        generation: *before_generation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_ports::FakeCancellation;
    use search_source_registry::api::{
        DEFAULT_REGISTRY_LIMITS, RegisterRootRequest, register_root,
    };
    use std::{
        fs::{self, File, OpenOptions},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };

    // Real redb, synthetic identities; no Native, credential or entropy calls.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "search-root-scope-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, create: bool) -> File {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(create)
                .open(self.0.join("control.redb"))
                .unwrap()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn identity() -> db::JournalIdentity {
        db::JournalIdentity {
            installation_incarnation_id: c::InstallationIncarnationId::from_bytes([2; 16]),
            data_root_id: c::DataRootId::from_bytes([3; 16]),
            owner_epoch: c::OwnerEpoch::new(1).unwrap(),
            path_identity_digest: c::Blake3Digest32::from_bytes([4; 32]),
            schema_family_digest: c::Blake3Digest32::from_bytes([5; 32]),
            schema_version: db::CURRENT_JOURNAL_SCHEMA_VERSION,
        }
    }
    fn bind<'a>(
        journal: &'a mut db::PersistentControlJournal,
        publisher: &'a mut db::ControlSnapshotPublisher,
        work: &'a dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> RegistryJournal<'a, FakeCancellation> {
        RegistryJournal::bind_original_v4(
            journal,
            publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
            Instant::now() + Duration::from_secs(60),
            work,
            search_source_admission::SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            search_source_admission::AcceptedCapabilities {
                profiles: vec![search_source_admission::BASELINE_PROFILE.into()],
            },
        )
        .unwrap()
    }
    fn assigned() -> RootSourceScope {
        let mut workspace = [21; 16];
        workspace[6] = 0x40;
        workspace[8] = 0x80;
        let mut corpus = [22; 16];
        corpus[6] = 0x40;
        corpus[8] = 0x80;
        RootSourceScope {
            workspace_id: c::WorkspaceId::from_bytes(workspace),
            corpus_id: c::CorpusId::from_bytes(corpus),
        }
    }

    #[test]
    fn scope_redb_reopen_replays_original_ids_without_obsolete_install_or_redraw() {
        let scratch = Scratch::new();
        let context = OperationContext::new(
            c::RequestId::from_bytes([1; 16]),
            60_000,
            FakeCancellation::new(false),
            c::OpaqueRef::new("scope-redb-proof").unwrap(),
        )
        .unwrap();
        let limits = db::JournalLimits::BASELINE;
        let mut journal =
            db::PersistentControlJournal::create(scratch.file(true), identity(), limits).unwrap();
        journal
            .initialize_operating_profile(
                db::JournalProfileInitialization::Direct,
                db::MutationId([9; 32]),
                c::Blake3Digest32::from_bytes([10; 32]),
                &context,
            )
            .unwrap();
        let mut publisher = db::ControlSnapshotPublisher::new();
        let work = |_| Ok::<_, OwnerError>(());
        let root_id = c::RootBindingId::from_bytes([13; 16]);
        let operation = c::OpaqueId::new("scope-original-root-setup").unwrap();
        let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).unwrap();
        {
            let mut port = bind(&mut journal, &mut publisher, &work);
            let request = RegisterRootRequest {
                root_binding_id: root_id,
                canonical_root_digest: c::Blake3Digest32::from_bytes([13; 32]),
                policy_fingerprint: c::Blake3Digest32::from_bytes([12; 32]),
                policy_revision: c::NonZeroRevision::new(1).unwrap(),
                owner_epoch: c::OwnerEpoch::new(1).unwrap(),
                owner_fence_receipt: c::ReceiptRef::new("scope-owner-fence").unwrap(),
                policy_receipt: c::ReceiptRef::new("scope-root-policy").unwrap(),
            };
            register_root(
                &mut registry,
                &request,
                &operation,
                c::Blake3Digest32::from_bytes([23; 32]),
                &c::ReceiptRef::new("scope-root-registered").unwrap(),
                &mut port,
                &context,
            )
            .unwrap();
            let root = registry.root(root_id).unwrap().clone();
            let observed = port
                .source_scope_with(&registry, &root, &context, &mut || Ok(()), |_, _| {
                    Ok(assigned())
                })
                .unwrap();
            assert_eq!(observed, assigned());
            // A later real C03 policy transaction must survive the original
            // scope replay; it is not an excuse to allocate a new scope.
            search_source_registry::api::update_root_policy(
                &mut registry,
                root_id,
                root.record_revision,
                c::Blake3Digest32::from_bytes([24; 32]),
                c::NonZeroRevision::new(2).unwrap(),
                &c::OpaqueId::new("scope-later-root-policy").unwrap(),
                c::Blake3Digest32::from_bytes([25; 32]),
                &c::ReceiptRef::new("scope-policy-updated").unwrap(),
                &mut port,
                &context,
            )
            .unwrap();
            assert!(
                port.source_scope_with(&registry, &root, &context, &mut || Ok(()), |_, _| panic!(
                    "stale Root cannot draw IDs"
                ))
                .is_err()
            );
        }
        let expected = registry.root(root_id).unwrap().clone();
        drop(journal);
        let mut journal =
            db::PersistentControlJournal::open(scratch.file(false), identity(), limits).unwrap();
        let writes = journal.committed_writes();
        {
            let mut port = bind(&mut journal, &mut publisher, &work);
            let restored = port.restore(&context).unwrap().into_registry();
            let root = restored.root(root_id).unwrap();
            assert_eq!(root, &expected);
            assert_eq!(
                port.source_scope_with(&restored, root, &context, &mut || Ok(()), |_, _| panic!(
                    "committed original must not draw IDs"
                ))
                .unwrap(),
                assigned()
            );
            assert_eq!(
                port.restore(&context)
                    .unwrap()
                    .registry()
                    .root(root_id)
                    .unwrap(),
                &expected
            );
            assert!(
                port.source_scope_with(
                    &restored,
                    root,
                    &context,
                    &mut || Err(OwnerError::OwnerOperationConflict),
                    |_, _| panic!("unqualified Root cannot draw IDs")
                )
                .is_err()
            );
        }
        assert_eq!(
            journal.committed_writes(),
            writes,
            "replay cannot write or install old state"
        );
    }
}
