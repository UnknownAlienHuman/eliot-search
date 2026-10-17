//! Root-owned C02 inventory, first-create recovery, and startup lease composition.
//!
//! This module is a child of the existing DIRECT registry owner. It borrows
//! that owner's one retained root and journal; it never opens or advances a
//! second root. Credential creation is reachable only through the explicit
//! one-shot setup API below. Ordinary startup uses the read-only lease method.

use super::super::read_child::{
    direct_operation,
    fresh_root::{FreshRootCancellation, FreshRootContext},
};
use super::{NativeProcessRootCustody, dek_command::NativeContext};
use search_config::SecretReference as ConfigSecretReference;
use search_contracts::{
    self as c, Blake3Digest32, BoundedBytes, BoundedList, CanonicalText, CanonicalValue as V,
    DataRootId, InstallationId, InstallationIncarnationId, NonZeroRevision, OpaqueId,
    blake3_canonical, parse_canonical_cbor, to_canonical_cbor,
};
use search_control_redb::{
    self as db, BoundControlJournal, CommitRecoveryDecision, ConditionalControlMutation,
    ControlKey, ControlMutation, ControlRecordClass, ControlRecordCondition, ControlValue,
    ControlWrite, JournalIdentity, JournalLimits, MigrationBootstrapWork as Work, MutationId,
    PersistentControlJournal, ScopedControlRecordsReadback,
};
use search_os_secrets_windows::{
    self as os, QdrantApiSecretLease, QdrantApiSecretObservation, QdrantApiSecretProvisioner,
    QdrantApiSecretPurpose, QdrantApiSecretReference, QdrantApiSecretStore,
};
use search_ports::{IdempotencyClass, MutationIdentity, OperationContext, SecretStorePort};
use search_runtime_owner::OwnerError;
use std::{
    fmt,
    time::{Duration, Instant},
};

const INVENTORY_RECORD_VERSION: u64 = 1;
const INVENTORY_KEY_PREFIX: &[u8] = b"eliot:qdrant-api-secret-inventory:v1:";

/// Exact, explicit initial-setup mutation. Persisted config does not create one.
pub(super) struct InitialQdrantSecretCreateIntent {
    operation: MutationIdentity,
}

impl InitialQdrantSecretCreateIntent {
    /// Binds setup to the stable operation ID of the explicit mutating command.
    pub(super) fn new(original_operation: &OpaqueId) -> Self {
        Self {
            operation: MutationIdentity::new(
                original_operation.clone(),
                IdempotencyClass::RetrySameIdentity,
            ),
        }
    }

    /// Retains an already constructed exact mutation identity from config apply.
    pub(super) fn from_mutation_identity(
        operation: MutationIdentity,
    ) -> Result<Self, &'static str> {
        if operation.idempotency != IdempotencyClass::RetrySameIdentity {
            return Err("QDRANT_SECRET_SETUP_IDEMPOTENCY_INVALID");
        }
        Ok(Self { operation })
    }
}

impl fmt::Debug for InitialQdrantSecretCreateIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InitialQdrantSecretCreateIntent(<opaque>)")
    }
}

/// The current C02-backed selected credential and its exact issuing context.
/// Parent startup moves `lease` and `context` into its `StartupSecret` value.
pub(super) struct QdrantStartupSecret {
    pub(super) lease: QdrantApiSecretLease<FreshRootCancellation>,
    pub(super) context: OperationContext<FreshRootCancellation>,
    pub(super) observation: QdrantApiSecretObservation,
}

impl fmt::Debug for QdrantStartupSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QdrantStartupSecret(<private lease>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InventoryStage {
    Intent,
    Dispatched,
    Current,
}

impl InventoryStage {
    const fn wire(self) -> u64 {
        match self {
            Self::Intent => 0,
            Self::Dispatched => 1,
            Self::Current => 2,
        }
    }

    const fn from_wire(value: u64) -> Result<Self, &'static str> {
        match value {
            0 => Ok(Self::Intent),
            1 => Ok(Self::Dispatched),
            2 => Ok(Self::Current),
            _ => Err("QDRANT_SECRET_INVENTORY_STAGE_INVALID"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InventoryBinding {
    data_root: DataRootId,
    installation: InstallationId,
    incarnation: InstallationIncarnationId,
    creator_epoch: u64,
    user_scope: Blake3Digest32,
    purpose: String,
    stable_reference: String,
    selection_digest: Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InventoryRecord {
    binding: InventoryBinding,
    original_operation: String,
    original_operation_digest: Blake3Digest32,
    stage: InventoryStage,
    version: NonZeroRevision,
    frame_digest: Option<Blake3Digest32>,
}

struct InventoryTransition<'a> {
    key: &'a ControlKey,
    expected_identity: JournalIdentity,
    snapshot: &'a ScopedControlRecordsReadback,
    before: Option<ControlValue>,
    after: ControlValue,
    original_operation: &'a MutationIdentity,
    phase: InventoryStage,
    deadline: Instant,
}

impl NativeProcessRootCustody {
    /// Explicit first-create operation. Ordinary startup must call
    /// `lease_current_qdrant_api_secret` and never infer setup from config.
    #[allow(clippy::too_many_lines)] // Keep C02 intent, dispatch marker and exact backend readback ordered together.
    pub(super) fn provision_initial_qdrant_api_secret(
        &mut self,
        selected: &ConfigSecretReference,
        setup: &InitialQdrantSecretCreateIntent,
    ) -> Result<QdrantApiSecretObservation, &'static str> {
        let identity = self.journal_identity;
        let root_owner = &mut self.owner.owner;
        let control = &mut self.owner.control;
        let mut operation =
            direct_operation(root_owner).map_err(|_| "QDRANT_SECRET_ROOT_CUSTODY_REFUSED")?;
        let native = operation
            .fresh_root_context()
            .map_err(|_| "QDRANT_SECRET_ROOT_CONTEXT_REFUSED")?;
        let journal_identity = control
            .journal
            .as_ref()
            .map(PersistentControlJournal::identity)
            .ok_or("QDRANT_SECRET_CONTROL_UNAVAILABLE")?;
        if journal_identity != identity {
            return Err("QDRANT_SECRET_CONTROL_OWNER_MISMATCH");
        }
        let facts = native
            .revision_dek_root_facts()
            .map_err(|_| "QDRANT_SECRET_ROOT_FACTS_REFUSED")?;
        if journal_identity.data_root_id != facts.data_root
            || journal_identity.installation_incarnation_id != facts.incarnation
            || journal_identity.owner_epoch != facts.creator_epoch
        {
            return Err("QDRANT_SECRET_ROOT_BINDING_MISMATCH");
        }
        let user_scope = observe_current_user(&native, native.original_deadline())?;
        native
            .source_full_checkpoint()
            .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
        let selected =
            selected_reference(selected, facts.installation, facts.incarnation, user_scope)?;
        let binding = inventory_binding(journal_identity, facts.creator_epoch.get(), &selected)?;
        let operation_digest = original_operation_digest(&binding, &setup.operation)?;
        let deadline = native.original_deadline();
        let work = |unit| native.work(unit);
        let mut port = bind(control, &native, deadline, &work)?;
        let key = inventory_key(&binding)?;
        let mut snapshot = read_inventory_snapshot(&port, &key, journal_identity, deadline, &work)?;
        let existing = decode_inventory_value(snapshot.get(&key))?;

        if let Some(record) = existing {
            verify_inventory_binding(&record, &binding, &setup.operation, operation_digest)?;
            match record.stage {
                InventoryStage::Current => {
                    return QdrantApiSecretProvisioner
                        .observe_exact(
                            &selected,
                            record.version,
                            record
                                .frame_digest
                                .ok_or("QDRANT_SECRET_INVENTORY_FRAME_MISSING")?,
                            &mut || remaining(&native, deadline),
                        )
                        .map_err(os::QdrantApiSecretStoreError::code);
                }
                InventoryStage::Dispatched => {
                    let observed = observe_dispatched(&native, deadline, &selected, &record)?;
                    let operation = identity_from_record(&record)?;
                    return finish_current(
                        &mut port,
                        &native,
                        &key,
                        journal_identity,
                        record,
                        observed,
                        &operation,
                        &selected,
                        deadline,
                        &work,
                    );
                }
                InventoryStage::Intent => {}
            }
        } else {
            // An existing OS slot without this Root's exact initial intent
            // is not adopted or overwritten.
            if QdrantApiSecretProvisioner
                .observe_selected(&selected, &mut || remaining(&native, deadline))
                .map_err(|error| match error {
                    os::QdrantApiSecretStoreError::SecretNotFound => "QDRANT_SECRET_NOT_FOUND",
                    other => other.code(),
                })?
                .is_some()
            {
                return Err("QDRANT_SECRET_UNOWNED_BACKEND_RECORD");
            }
            let record = InventoryRecord {
                binding: binding.clone(),
                original_operation: setup.operation.operation_id.as_str().to_owned(),
                original_operation_digest: operation_digest,
                stage: InventoryStage::Intent,
                version: NonZeroRevision::new(1).map_err(|_| "QDRANT_SECRET_VERSION_INVALID")?,
                frame_digest: None,
            };
            let value = inventory_value(&record)?;
            write_inventory_phase(
                &mut port,
                &native,
                InventoryTransition {
                    key: &key,
                    expected_identity: journal_identity,
                    snapshot: &snapshot,
                    before: None,
                    after: value,
                    original_operation: &setup.operation,
                    phase: InventoryStage::Intent,
                    deadline,
                },
                &work,
            )?;
            snapshot = read_inventory_snapshot(&port, &key, journal_identity, deadline, &work)?;
            if snapshot.get(&key) != Some(&inventory_value(&record)?) {
                return Err("QDRANT_SECRET_CREATE_INTENT_READBACK_MISMATCH");
            }
        }

        let record = decode_inventory_value(snapshot.get(&key))?
            .ok_or("QDRANT_SECRET_CREATE_INTENT_MISSING")?;
        verify_inventory_binding(&record, &binding, &setup.operation, operation_digest)?;
        if record.stage != InventoryStage::Intent {
            return Err("QDRANT_SECRET_SETUP_STATE_CONFLICT");
        }

        let mut remaining = || remaining(&native, deadline);
        let prepared = QdrantApiSecretProvisioner
            .prepare_initial(&selected, &mut remaining)
            .map_err(os::QdrantApiSecretStoreError::code)?;
        if prepared.selected() != &selected || prepared.version() != record.version {
            return Err("QDRANT_SECRET_PREPARED_BINDING_MISMATCH");
        }
        let dispatching = InventoryRecord {
            stage: InventoryStage::Dispatched,
            frame_digest: Some(prepared.frame_digest()),
            ..record
        };
        let before = snapshot
            .get(&key)
            .cloned()
            .ok_or("QDRANT_SECRET_CREATE_INTENT_MISSING")?;
        let next = inventory_value(&dispatching)?;
        write_inventory_phase(
            &mut port,
            &native,
            InventoryTransition {
                key: &key,
                expected_identity: journal_identity,
                snapshot: &snapshot,
                before: Some(before),
                after: next,
                original_operation: &setup.operation,
                phase: InventoryStage::Dispatched,
                deadline,
            },
            &work,
        )?;
        snapshot = read_inventory_snapshot(&port, &key, journal_identity, deadline, &work)?;
        if snapshot.get(&key) != Some(&inventory_value(&dispatching)?) {
            return Err("QDRANT_SECRET_DISPATCH_INTENT_READBACK_MISMATCH");
        }

        let observed = QdrantApiSecretProvisioner
            .dispatch_prepared(
                prepared,
                dispatching
                    .frame_digest
                    .ok_or("QDRANT_SECRET_INVENTORY_FRAME_MISSING")?,
                &mut remaining,
            )
            .map_err(os::QdrantApiSecretStoreError::code)?;
        if observed.selected() != &selected
            || observed.version() != dispatching.version
            || Some(observed.frame_digest()) != dispatching.frame_digest
        {
            return Err("QDRANT_SECRET_BACKEND_READBACK_MISMATCH");
        }
        finish_current(
            &mut port,
            &native,
            &key,
            journal_identity,
            dispatching,
            observed,
            &setup.operation,
            &selected,
            deadline,
            &work,
        )
    }

    /// Read-only startup path: require exact C02 Current inventory, then lease
    /// the exact Windows frame under the same retained Root request context.
    #[allow(clippy::too_many_lines)] // Keep exact C02 recovery, OS readback and lease issuance in one checked request.
    pub(super) fn lease_current_qdrant_api_secret(
        &mut self,
        selected_config_ref: &ConfigSecretReference,
        lease_ttl_ms: u64,
    ) -> Result<QdrantStartupSecret, &'static str> {
        let pending_setup = {
            let identity = self.journal_identity;
            let root_owner = &mut self.owner.owner;
            let control = &mut self.owner.control;
            let mut root_operation =
                direct_operation(root_owner).map_err(|_| "QDRANT_SECRET_ROOT_CUSTODY_REFUSED")?;
            let native = root_operation
                .fresh_root_context()
                .map_err(|_| "QDRANT_SECRET_ROOT_CONTEXT_REFUSED")?;
            let journal_identity = control
                .journal
                .as_ref()
                .map(PersistentControlJournal::identity)
                .ok_or("QDRANT_SECRET_CONTROL_UNAVAILABLE")?;
            if journal_identity != identity {
                return Err("QDRANT_SECRET_CONTROL_OWNER_MISMATCH");
            }
            let facts = native
                .revision_dek_root_facts()
                .map_err(|_| "QDRANT_SECRET_ROOT_FACTS_REFUSED")?;
            if journal_identity.data_root_id != facts.data_root
                || journal_identity.installation_incarnation_id != facts.incarnation
                || journal_identity.owner_epoch != facts.creator_epoch
            {
                return Err("QDRANT_SECRET_ROOT_BINDING_MISMATCH");
            }
            let user_scope = observe_current_user(&native, native.original_deadline())?;
            native
                .source_full_checkpoint()
                .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
            let selected = selected_reference(
                selected_config_ref,
                facts.installation,
                facts.incarnation,
                user_scope,
            )?;
            let binding =
                inventory_binding(journal_identity, facts.creator_epoch.get(), &selected)?;
            let deadline = native.original_deadline();
            let work = |unit| native.work(unit);
            let mut port = bind(control, &native, deadline, &work)?;
            let key = inventory_key(&binding)?;
            let snapshot = read_inventory_snapshot(&port, &key, journal_identity, deadline, &work)?;
            let record = decode_inventory_value(snapshot.get(&key))?
                .ok_or("QDRANT_SECRET_INVENTORY_MISSING")?;
            let original_operation = identity_from_record(&record)?;
            let operation_digest = original_operation_digest(&binding, &original_operation)?;
            verify_inventory_binding(&record, &binding, &original_operation, operation_digest)?;

            if record.stage == InventoryStage::Intent {
                // The persisted exact create intent is the only authority to
                // resume pre-dispatch setup; never substitute a fresh ID.
                Some(original_operation)
            } else {
                if record.stage == InventoryStage::Dispatched {
                    let observed = observe_dispatched(&native, deadline, &selected, &record)?;
                    finish_current(
                        &mut port,
                        &native,
                        &key,
                        journal_identity,
                        record,
                        observed,
                        &original_operation,
                        &selected,
                        deadline,
                        &work,
                    )?;
                }
                let snapshot =
                    read_inventory_snapshot(&port, &key, journal_identity, deadline, &work)?;
                let record = decode_inventory_value(snapshot.get(&key))?
                    .ok_or("QDRANT_SECRET_INVENTORY_MISSING")?;
                verify_inventory_binding_only(&record, &binding)?;
                if record.stage != InventoryStage::Current {
                    return Err("QDRANT_SECRET_INVENTORY_STATE_INVALID");
                }
                let expected_digest = record
                    .frame_digest
                    .ok_or("QDRANT_SECRET_INVENTORY_FRAME_MISSING")?;
                let observation = QdrantApiSecretProvisioner
                    .observe_exact(&selected, record.version, expected_digest, &mut || {
                        remaining(&native, deadline)
                    })
                    .map_err(os::QdrantApiSecretStoreError::code)?;

                let context = remaining_operation_context(&native, deadline)?;
                let store = QdrantApiSecretStore::<FreshRootCancellation>::new(lease_ttl_ms)
                    .map_err(os::QdrantApiSecretStoreError::code)?;
                let lease = store
                    .lease_secret(&selected, &QdrantApiSecretPurpose::new(), &context)
                    .map_err(os::QdrantApiSecretStoreError::code)?;
                lease
                    .with_secret_live_window(
                        &context,
                        |_, reference, frame_digest, observed, expires| {
                            let lease_reference = reference.id().as_str();
                            let selected_reference = selected.stable_reference().as_str();
                            if lease_reference != selected_reference
                                || reference.binding() != selected.binding()
                                || reference.version() != observation.version()
                                || frame_digest != observation.frame_digest()
                                || observed.get() >= expires.get()
                            {
                                Err("QDRANT_SECRET_LEASE_READBACK_MISMATCH")
                            } else {
                                Ok(())
                            }
                        },
                    )
                    .map_err(os::QdrantApiSecretStoreError::code)??;
                return Ok(QdrantStartupSecret {
                    lease,
                    context,
                    observation,
                });
            }
        };

        let setup = InitialQdrantSecretCreateIntent::from_mutation_identity(
            pending_setup.ok_or("QDRANT_SECRET_SETUP_RECOVERY_REQUIRED")?,
        )?;
        self.provision_initial_qdrant_api_secret(selected_config_ref, &setup)?;
        self.lease_current_qdrant_api_secret(selected_config_ref, lease_ttl_ms)
    }

    /// Ensures the configured Qdrant credential for this explicit mutating
    /// command. Initial creation is allowed only when C02 confirms that no
    /// inventory has ever bound this selected reference.
    pub(super) fn ensure_qdrant_api_secret_for_command(
        &mut self,
        config: &crate::config_composition::EffectiveDaemonConfig,
        original_operation: &OpaqueId,
    ) -> Result<QdrantStartupSecret, &'static str> {
        let settings = config.native_qdrant_settings();
        if !settings.enabled {
            return Err("NATIVE_QDRANT_DISABLED");
        }
        let selected = settings
            .api_secret_ref
            .as_ref()
            .ok_or("QDRANT_SECRET_REFERENCE_MISSING")?;
        let lease_ttl_ms = config.secrets_settings().lease_ttl_ms();
        match self.lease_current_qdrant_api_secret(selected, lease_ttl_ms) {
            Ok(secret) => Ok(secret),
            Err("QDRANT_SECRET_INVENTORY_MISSING") => {
                let setup = InitialQdrantSecretCreateIntent::new(original_operation);
                self.provision_initial_qdrant_api_secret(selected, &setup)?;
                self.lease_current_qdrant_api_secret(selected, lease_ttl_ms)
            }
            Err(reason) => Err(reason),
        }
    }
}

fn bind<'journal, F>(
    control: &'journal mut super::DirectJournal,
    native: &FreshRootContext<'_, '_>,
    deadline: Instant,
    work: &F,
) -> Result<BoundControlJournal<'journal, FreshRootCancellation>, &'static str>
where
    F: Fn(Work) -> Result<(), OwnerError>,
{
    native
        .source_full_checkpoint()
        .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
    let journal = control
        .journal
        .as_mut()
        .ok_or("QDRANT_SECRET_CONTROL_UNAVAILABLE")?;
    let port = journal
        .bind_control_port_with_original_context::<FreshRootCancellation, OwnerError>(
            &mut control.publisher,
            deadline,
            work,
        )
        .map_err(|_| "QDRANT_SECRET_CONTROL_BIND_REFUSED")?;
    native
        .source_full_checkpoint()
        .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
    Ok(port)
}

fn read_inventory_snapshot<F>(
    port: &BoundControlJournal<'_, FreshRootCancellation>,
    key: &ControlKey,
    expected_identity: JournalIdentity,
    deadline: Instant,
    work: &F,
) -> Result<ScopedControlRecordsReadback, &'static str>
where
    F: Fn(Work) -> Result<(), OwnerError>,
{
    let snapshot = port
        .read_control_keys_with_original_context(std::slice::from_ref(key), deadline, work)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_READ_REFUSED")?;
    if snapshot.identity() != expected_identity {
        return Err("QDRANT_SECRET_CONTROL_OWNER_MISMATCH");
    }
    if snapshot.get(key).is_none() && !snapshot.missing_keys().contains(key) {
        return Err("QDRANT_SECRET_INVENTORY_READBACK_INCOMPLETE");
    }
    Ok(snapshot)
}

fn decode_inventory_value(
    value: Option<&ControlValue>,
) -> Result<Option<InventoryRecord>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.class() != ControlRecordClass::State {
        return Err("QDRANT_SECRET_INVENTORY_CLASS_INVALID");
    }
    decode_inventory(value.as_bytes()).map(Some)
}

fn selected_reference(
    selected: &ConfigSecretReference,
    installation: InstallationId,
    incarnation: InstallationIncarnationId,
    user_scope: Blake3Digest32,
) -> Result<QdrantApiSecretReference, &'static str> {
    let stable = OpaqueId::new(selected.as_str().to_owned())
        .map_err(|_| "QDRANT_SECRET_REFERENCE_INVALID")?;
    QdrantApiSecretReference::for_owner(stable, installation, incarnation, user_scope)
        .map_err(os::QdrantApiSecretStoreError::code)
}

fn inventory_binding(
    identity: JournalIdentity,
    creator_epoch: u64,
    selected: &QdrantApiSecretReference,
) -> Result<InventoryBinding, &'static str> {
    if creator_epoch == 0
        || identity.owner_epoch.get() != creator_epoch
        || identity.installation_incarnation_id != selected.binding().installation_incarnation_id()
    {
        return Err("QDRANT_SECRET_ROOT_BINDING_MISMATCH");
    }
    Ok(InventoryBinding {
        data_root: identity.data_root_id,
        installation: selected.binding().installation_id(),
        incarnation: identity.installation_incarnation_id,
        creator_epoch,
        user_scope: selected.binding().user_scope_digest(),
        purpose: selected.binding().purpose().as_str().to_owned(),
        stable_reference: selected.stable_reference().as_str().to_owned(),
        selection_digest: selected.selection_digest(),
    })
}

fn verify_inventory_binding_only(
    record: &InventoryRecord,
    current: &InventoryBinding,
) -> Result<(), &'static str> {
    let stored = &record.binding;
    if stored.data_root != current.data_root
        || stored.installation != current.installation
        || stored.incarnation != current.incarnation
        || stored.user_scope != current.user_scope
        || stored.purpose != current.purpose
        || stored.stable_reference != current.stable_reference
        || stored.selection_digest != current.selection_digest
    {
        return Err("QDRANT_SECRET_INVENTORY_BINDING_MISMATCH");
    }
    Ok(())
}

fn verify_inventory_binding(
    record: &InventoryRecord,
    current: &InventoryBinding,
    operation: &MutationIdentity,
    operation_digest: Blake3Digest32,
) -> Result<(), &'static str> {
    verify_inventory_binding_only(record, current)?;
    if record.original_operation != operation.operation_id.as_str()
        || record.original_operation_digest != operation_digest
    {
        return Err("QDRANT_SECRET_SETUP_OPERATION_CONFLICT");
    }
    Ok(())
}

fn inventory_key(binding: &InventoryBinding) -> Result<ControlKey, &'static str> {
    // SID is retained and compared in the value. Keeping the key stable across
    // user changes lets startup detect and refuse an old-user binding instead
    // of treating it as a fresh slot under the same configured reference.
    let digest = hash(
        "eliot/cbor/qdrant-api-secret-inventory-key/v1",
        vec![
            bytes(binding.data_root.as_bytes())?,
            bytes(binding.installation.as_bytes())?,
            bytes(binding.incarnation.as_bytes())?,
            text(&binding.stable_reference)?,
        ],
    )?;
    let mut key = Vec::with_capacity(INVENTORY_KEY_PREFIX.len() + 32);
    key.extend_from_slice(INVENTORY_KEY_PREFIX);
    key.extend_from_slice(digest.as_bytes());
    ControlKey::new(key, JournalLimits::BASELINE).map_err(|_| "QDRANT_SECRET_INVENTORY_KEY_INVALID")
}

fn original_operation_digest(
    binding: &InventoryBinding,
    operation: &MutationIdentity,
) -> Result<Blake3Digest32, &'static str> {
    hash(
        "eliot/cbor/qdrant-api-secret-create-intent/v1",
        vec![
            text(operation.operation_id.as_str())?,
            bytes(binding.data_root.as_bytes())?,
            bytes(binding.installation.as_bytes())?,
            bytes(binding.incarnation.as_bytes())?,
            bytes(binding.user_scope.as_bytes())?,
            text(&binding.purpose)?,
            text(&binding.stable_reference)?,
            bytes(binding.selection_digest.as_bytes())?,
        ],
    )
}

fn inventory_value(record: &InventoryRecord) -> Result<ControlValue, &'static str> {
    validate_inventory_record(record)?;
    let frame = record
        .frame_digest
        .map(|digest| bytes(digest.as_bytes()))
        .transpose()?
        .unwrap_or(V::Null);
    let fields = vec![
        text("eliot.qdrant-api-secret.inventory.v1")?,
        V::U64(INVENTORY_RECORD_VERSION),
        V::U64(record.stage.wire()),
        bytes(record.binding.data_root.as_bytes())?,
        bytes(record.binding.installation.as_bytes())?,
        bytes(record.binding.incarnation.as_bytes())?,
        V::U64(record.binding.creator_epoch),
        bytes(record.binding.user_scope.as_bytes())?,
        text(&record.binding.purpose)?,
        text(&record.binding.stable_reference)?,
        bytes(record.binding.selection_digest.as_bytes())?,
        text(&record.original_operation)?,
        bytes(record.original_operation_digest.as_bytes())?,
        V::U64(record.version.get()),
        frame,
    ];
    let encoded =
        to_canonical_cbor(&array(fields)?).map_err(|_| "QDRANT_SECRET_INVENTORY_ENCODE_REFUSED")?;
    ControlValue::new(
        ControlRecordClass::State,
        encoded.as_slice().to_vec(),
        JournalLimits::BASELINE,
    )
    .map_err(|_| "QDRANT_SECRET_INVENTORY_VALUE_INVALID")
}

fn decode_inventory(bytes: &[u8]) -> Result<InventoryRecord, &'static str> {
    let value = parse_canonical_cbor(bytes).map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_INVALID")?;
    let V::Array(fields) = value else {
        return Err("QDRANT_SECRET_INVENTORY_CODEC_INVALID");
    };
    let fields = fields.as_slice();
    if fields.len() != 15
        || text_value(&fields[0])? != "eliot.qdrant-api-secret.inventory.v1"
        || u64_value(&fields[1])? != INVENTORY_RECORD_VERSION
    {
        return Err("QDRANT_SECRET_INVENTORY_CODEC_INVALID");
    }
    let data_root = DataRootId::from_bytes(uuid_bytes(&fields[3])?);
    let installation = InstallationId::from_bytes(uuid_bytes(&fields[4])?);
    let incarnation = InstallationIncarnationId::from_bytes(uuid_bytes(&fields[5])?);
    let creator_epoch = u64_value(&fields[6])?;
    if creator_epoch == 0 {
        return Err("QDRANT_SECRET_INVENTORY_CREATOR_EPOCH_INVALID");
    }
    let user_scope = digest32(&fields[7])?;
    let purpose = text_value(&fields[8])?.to_owned();
    let stable_reference = text_value(&fields[9])?.to_owned();
    let selection_digest = digest32(&fields[10])?;
    let original_operation = text_value(&fields[11])?.to_owned();
    OpaqueId::new(original_operation.clone())
        .map_err(|_| "QDRANT_SECRET_INVENTORY_OPERATION_INVALID")?;
    let original_operation_digest = digest32(&fields[12])?;
    let version = NonZeroRevision::new(u64_value(&fields[13])?)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_VERSION_INVALID")?;
    let frame_digest = match &fields[14] {
        V::Null => None,
        value => Some(digest32(value)?),
    };
    let stage = InventoryStage::from_wire(u64_value(&fields[2])?)?;
    let record = InventoryRecord {
        binding: InventoryBinding {
            data_root,
            installation,
            incarnation,
            creator_epoch,
            user_scope,
            purpose,
            stable_reference,
            selection_digest,
        },
        original_operation,
        original_operation_digest,
        stage,
        version,
        frame_digest,
    };
    validate_inventory_record(&record)?;
    Ok(record)
}

fn validate_inventory_record(record: &InventoryRecord) -> Result<(), &'static str> {
    if record.version.get() != 1
        || record.binding.creator_epoch == 0
        || (record.stage == InventoryStage::Intent) != record.frame_digest.is_none()
        || OpaqueId::new(record.original_operation.clone()).is_err()
    {
        return Err("QDRANT_SECRET_INVENTORY_STATE_INVALID");
    }
    Ok(())
}

fn write_inventory_phase<F>(
    port: &mut BoundControlJournal<'_, FreshRootCancellation>,
    native: &FreshRootContext<'_, '_>,
    transition: InventoryTransition<'_>,
    work: &F,
) -> Result<(), &'static str>
where
    F: Fn(Work) -> Result<(), OwnerError>,
{
    let InventoryTransition {
        key,
        expected_identity,
        snapshot,
        before,
        after,
        original_operation,
        phase,
        deadline,
    } = transition;
    if snapshot.identity() != expected_identity || snapshot.generation() == u64::MAX {
        return Err("QDRANT_SECRET_CONTROL_GENERATION_REFUSED");
    }
    match before.as_ref() {
        Some(value) if snapshot.get(key) == Some(value) => {}
        None if snapshot.get(key).is_none() && snapshot.missing_keys().contains(key) => {}
        _ => return Err("QDRANT_SECRET_INVENTORY_PRESTATE_CHANGED"),
    }
    if after.class() != ControlRecordClass::State {
        return Err("QDRANT_SECRET_INVENTORY_CLASS_INVALID");
    }
    let id = phase_id(original_operation, key, phase)?;
    let command_digest = phase_command_digest(id, snapshot.generation(), before.as_ref(), &after)?;
    let mutation = ControlMutation::new(
        id,
        command_digest,
        snapshot.generation(),
        vec![ControlWrite {
            key: key.clone(),
            value: after,
        }],
        Vec::new(),
    );
    let condition = before.map_or_else(
        || ControlRecordCondition::absent(key.clone()),
        |value| ControlRecordCondition::exact(key.clone(), value),
    );
    let command = ConditionalControlMutation::new(mutation, vec![condition]);
    let identity =
        db::control_mutation_identity(id).map_err(|_| "QDRANT_SECRET_CONTROL_IDENTITY_INVALID")?;
    native
        .source_full_checkpoint()
        .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
    let committed =
        match port.transact_records_with_original_context(&command, &identity, deadline, work) {
            Ok(receipt) => receipt,
            Err(_) => match port
                .recover_records_with_original_context(&command, &identity, deadline, work)
            {
                Ok(CommitRecoveryDecision::Committed(receipt)) => receipt,
                Ok(CommitRecoveryDecision::NotCommittedRetrySameOperation) => {
                    match port
                        .transact_records_with_original_context(&command, &identity, deadline, work)
                    {
                        Ok(receipt) => receipt,
                        Err(_) => match port.recover_records_with_original_context(
                            &command, &identity, deadline, work,
                        ) {
                            Ok(CommitRecoveryDecision::Committed(receipt)) => receipt,
                            _ => return Err("QDRANT_SECRET_CONTROL_OUTCOME_UNKNOWN"),
                        },
                    }
                }
                _ => return Err("QDRANT_SECRET_CONTROL_OUTCOME_UNKNOWN"),
            },
        };
    if committed.operation_id != id || committed.command_digest != command_digest {
        return Err("QDRANT_SECRET_CONTROL_RECEIPT_MISMATCH");
    }
    native
        .source_full_checkpoint()
        .map_err(|_| "QDRANT_SECRET_ROOT_CHECKPOINT_REFUSED")?;
    Ok(())
}

fn phase_id(
    operation: &MutationIdentity,
    key: &ControlKey,
    phase: InventoryStage,
) -> Result<MutationId, &'static str> {
    let digest = hash(
        "eliot/cbor/qdrant-api-secret-inventory-operation/v1",
        vec![
            text(operation.operation_id.as_str())?,
            bytes(key.as_bytes())?,
            V::U64(phase.wire()),
        ],
    )?;
    Ok(MutationId(*digest.as_bytes()))
}

fn phase_command_digest(
    id: MutationId,
    generation: u64,
    before: Option<&ControlValue>,
    after: &ControlValue,
) -> Result<Blake3Digest32, &'static str> {
    hash(
        "eliot/cbor/qdrant-api-secret-inventory-command/v1",
        vec![
            bytes(&id.0)?,
            V::U64(generation),
            before.map_or(Ok(V::Null), |value| bytes(value.as_bytes()))?,
            bytes(after.as_bytes())?,
        ],
    )
}

fn observe_dispatched(
    native: &FreshRootContext<'_, '_>,
    deadline: Instant,
    selected: &QdrantApiSecretReference,
    record: &InventoryRecord,
) -> Result<QdrantApiSecretObservation, &'static str> {
    if record.stage != InventoryStage::Dispatched {
        return Err("QDRANT_SECRET_INVENTORY_STATE_INVALID");
    }
    let digest = record
        .frame_digest
        .ok_or("QDRANT_SECRET_INVENTORY_FRAME_MISSING")?;
    QdrantApiSecretProvisioner
        .observe_exact(selected, record.version, digest, &mut || {
            remaining(native, deadline)
        })
        .map_err(|error| match error {
            os::QdrantApiSecretStoreError::SecretNotFound => {
                "QDRANT_SECRET_DISPATCHED_CREDENTIAL_MISSING"
            }
            os::QdrantApiSecretStoreError::CredentialFrameMismatch => {
                "QDRANT_SECRET_DISPATCHED_FRAME_MISMATCH"
            }
            other => other.code(),
        })
}

#[allow(clippy::too_many_arguments)]
fn finish_current<F>(
    port: &mut BoundControlJournal<'_, FreshRootCancellation>,
    native: &FreshRootContext<'_, '_>,
    key: &ControlKey,
    expected_identity: JournalIdentity,
    dispatched: InventoryRecord,
    observed: QdrantApiSecretObservation,
    operation: &MutationIdentity,
    selected: &QdrantApiSecretReference,
    deadline: Instant,
    work: &F,
) -> Result<QdrantApiSecretObservation, &'static str>
where
    F: Fn(Work) -> Result<(), OwnerError>,
{
    if dispatched.stage != InventoryStage::Dispatched
        || observed.selected() != selected
        || observed.version() != dispatched.version
        || Some(observed.frame_digest()) != dispatched.frame_digest
    {
        return Err("QDRANT_SECRET_BACKEND_READBACK_MISMATCH");
    }
    let identity = port
        .read_control_keys_with_original_context(std::slice::from_ref(key), deadline, work)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_READ_REFUSED")?;
    if identity.identity() != expected_identity {
        return Err("QDRANT_SECRET_CONTROL_OWNER_MISMATCH");
    }
    let expected = inventory_value(&dispatched)?;
    if identity.get(key) != Some(&expected) {
        return Err("QDRANT_SECRET_INVENTORY_PRESTATE_CHANGED");
    }
    let current = InventoryRecord {
        stage: InventoryStage::Current,
        ..dispatched
    };
    let next = inventory_value(&current)?;
    write_inventory_phase(
        port,
        native,
        InventoryTransition {
            key,
            expected_identity,
            snapshot: &identity,
            before: Some(expected),
            after: next.clone(),
            original_operation: operation,
            phase: InventoryStage::Current,
            deadline,
        },
        work,
    )?;
    let readback = port
        .read_control_keys_with_original_context(std::slice::from_ref(key), deadline, work)
        .map_err(|_| "QDRANT_SECRET_CURRENT_READBACK_REFUSED")?;
    if readback.identity() != expected_identity || readback.get(key) != Some(&next) {
        return Err("QDRANT_SECRET_CURRENT_READBACK_MISMATCH");
    }
    Ok(observed)
}

fn identity_from_record(record: &InventoryRecord) -> Result<MutationIdentity, &'static str> {
    let operation = OpaqueId::new(record.original_operation.clone())
        .map_err(|_| "QDRANT_SECRET_INVENTORY_OPERATION_INVALID")?;
    Ok(MutationIdentity::new(
        operation,
        IdempotencyClass::RetrySameIdentity,
    ))
}

fn remaining(native: &FreshRootContext<'_, '_>, deadline: Instant) -> Option<Duration> {
    native.source_full_checkpoint().ok()?;
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
}

fn remaining_operation_context(
    native: &FreshRootContext<'_, '_>,
    deadline: Instant,
) -> Result<OperationContext<FreshRootCancellation>, &'static str> {
    let remaining = remaining(native, deadline).ok_or("QDRANT_SECRET_CONTEXT_EXPIRED")?;
    let relative_ms = u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX);
    if relative_ms == 0 {
        return Err("QDRANT_SECRET_CONTEXT_EXPIRED");
    }
    let original = native.context();
    OperationContext::new(
        original.request_id(),
        relative_ms,
        original.cancellation().clone(),
        original.budget_ref().clone(),
    )
    .map_err(|_| "QDRANT_SECRET_CONTEXT_INVALID")
}

fn observe_current_user(
    native: &FreshRootContext<'_, '_>,
    deadline: Instant,
) -> Result<Blake3Digest32, &'static str> {
    let poll = || native.work(Work::Checkpoint);
    let native_work = |unit| match unit {
        os::LegacyRevisionDpapiWork::NativeCall => native.revision_dek_native_attempt(),
        os::LegacyRevisionDpapiWork::Allocate(bytes) => native.work(Work::Allocate(bytes)),
        os::LegacyRevisionDpapiWork::Process(bytes) => native.work(Work::Process(bytes)),
    };
    let open = || native.work(Work::Open);
    let clear = |bytes| native.revision_dek_account_native_clear(bytes);
    let mut context = NativeContext {
        deadline,
        poll: &poll,
        work: &native_work,
        open: &open,
        clear: &clear,
    };
    let observed = os::observe_revision_user(&mut context).map_err(|error| error.code())?;
    Ok(Blake3Digest32::from_bytes(observed.user_scope_digest()))
}

fn array(fields: Vec<V>) -> Result<V, &'static str> {
    BoundedList::new(fields)
        .map(V::Array)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_BOUNDS")
}

fn text(value: &str) -> Result<V, &'static str> {
    CanonicalText::new_non_empty(value.to_owned())
        .map(V::Text)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_BOUNDS")
}

fn bytes(value: &[u8]) -> Result<V, &'static str> {
    BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_BOUNDS")
}

fn hash(domain: &str, fields: Vec<V>) -> Result<Blake3Digest32, &'static str> {
    let value = array(fields)?;
    let domain = c::CanonicalDigestDomain::parse(domain)
        .map_err(|_| "QDRANT_SECRET_INVENTORY_DIGEST_DOMAIN_INVALID")?;
    blake3_canonical(
        &domain,
        &value,
        c::DigestInputLimit::new(16 * 1024).map_err(|_| "QDRANT_SECRET_INVENTORY_DIGEST_BOUNDS")?,
    )
    .map_err(|_| "QDRANT_SECRET_INVENTORY_DIGEST_REFUSED")
}

fn text_value(value: &V) -> Result<&str, &'static str> {
    match value {
        V::Text(value) => Ok(value.as_str()),
        _ => Err("QDRANT_SECRET_INVENTORY_CODEC_INVALID"),
    }
}

fn bytes_value(value: &V) -> Result<&[u8], &'static str> {
    match value {
        V::Bytes(value) => Ok(value.as_slice()),
        _ => Err("QDRANT_SECRET_INVENTORY_CODEC_INVALID"),
    }
}

const fn u64_value(value: &V) -> Result<u64, &'static str> {
    match value {
        V::U64(value) => Ok(*value),
        _ => Err("QDRANT_SECRET_INVENTORY_CODEC_INVALID"),
    }
}

fn uuid_bytes(value: &V) -> Result<[u8; 16], &'static str> {
    bytes_value(value)?
        .try_into()
        .map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_INVALID")
}

fn digest32(value: &V) -> Result<Blake3Digest32, &'static str> {
    let bytes: [u8; 32] = bytes_value(value)?
        .try_into()
        .map_err(|_| "QDRANT_SECRET_INVENTORY_CODEC_INVALID")?;
    Ok(Blake3Digest32::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(user_scope: u8) -> InventoryBinding {
        let stable_reference = "secret://synthetic/qdrant".to_owned();
        let user_scope = Blake3Digest32::from_bytes([user_scope; 32]);
        let selection_digest = Blake3Digest32::from_bytes([0x71; 32]);
        InventoryBinding {
            data_root: DataRootId::from_bytes([0x11; 16]),
            installation: InstallationId::from_bytes([0x22; 16]),
            incarnation: InstallationIncarnationId::from_bytes([0x33; 16]),
            creator_epoch: 7,
            user_scope,
            purpose: "secret-purpose:qdrant-api-key/v1".to_owned(),
            stable_reference,
            selection_digest,
        }
    }

    fn record(stage: InventoryStage) -> InventoryRecord {
        InventoryRecord {
            binding: binding(0x44),
            original_operation: "qdrant-secret-setup-fixture".to_owned(),
            original_operation_digest: Blake3Digest32::from_bytes([0x55; 32]),
            stage,
            version: NonZeroRevision::new(1).expect("nonzero version"),
            frame_digest: (stage != InventoryStage::Intent)
                .then(|| Blake3Digest32::from_bytes([0x66; 32])),
        }
    }

    #[test]
    fn inventory_codec_roundtrips_intent_dispatch_and_current_without_secret_bytes() {
        for stage in [
            InventoryStage::Intent,
            InventoryStage::Dispatched,
            InventoryStage::Current,
        ] {
            let expected = record(stage);
            let encoded = inventory_value(&expected).expect("bounded canonical inventory");
            assert_eq!(
                decode_inventory(encoded.as_bytes()).expect("strict decode"),
                expected
            );
        }
    }

    #[test]
    fn stable_slot_key_detects_user_scope_change_instead_of_creating_an_alias() {
        let original = binding(0x44);
        let changed_user = binding(0x45);
        assert_eq!(
            inventory_key(&original).expect("original slot key"),
            inventory_key(&changed_user).expect("same configured slot key")
        );
        assert_eq!(
            verify_inventory_binding_only(&record(InventoryStage::Current), &changed_user),
            Err("QDRANT_SECRET_INVENTORY_BINDING_MISMATCH")
        );
    }
}
