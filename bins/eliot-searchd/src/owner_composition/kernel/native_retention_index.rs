//! Ordinary lexical ingestion through the same owned runtime and source callback.

use super::super::read_child::fresh_root::FreshRootCancellation;
#[cfg(windows)]
use super::{
    DirectRegistryOwner,
    indexed_runtime::{self, StartupSecret},
    retention_command::NativeRetentionCommand,
    retention_publication::IndexedRetention,
};
use super::{
    QualifiedNativeProfiles,
    command::{original_port, published_metadata},
    index_bootstrap::{self, IndexBootstrapDisposition},
    retention_control::phase_id,
    retention_source::RetentionSourceInput,
};
#[cfg(windows)]
use crate::{
    config_composition::{EffectiveDaemonConfig, RootLocatorHints},
    owner_composition::kernel::direct_policy_v2::CapturedPolicyV2,
};
use search_contracts::{Epoch, OpaqueId};
use search_control_redb::{
    CommitRecoveryDecision, ControlSnapshotPublisher, PersistentControlJournal,
    PublicationVisibilityState,
};
use search_qdrant_bridge::{
    BridgeError, CollectionRoute,
    real::{BlockingRealQueryPlane, CanonicalRequestContext},
};
use std::sync::Arc;
#[cfg(windows)]
use std::{io::Write, path::Path};

#[cfg(windows)]
impl DirectRegistryOwner {
    pub(super) fn execute_retention_lexical(
        self,
        hints: &RootLocatorHints,
        path: &Path,
        policy: &CapturedPolicyV2,
        command: &NativeRetentionCommand,
        config: &EffectiveDaemonConfig,
        writer: &mut impl Write,
    ) -> Result<(), String> {
        let result = indexed_runtime::with_owned_indexed(
            self,
            config,
            |custody, config| {
                let secret = custody
                    .ensure_qdrant_api_secret_for_command(config, &command.operation)
                    .map_err(str::to_owned)?;
                Ok(StartupSecret {
                    lease: secret.lease,
                    context: secret.context,
                    observation: secret.observation,
                })
            },
            |custody, plane, profiles| {
                let output = custody.owner.with_retention_source(
                    hints,
                    path,
                    policy,
                    command,
                    |input, journal, publisher| {
                        let route = ensure_generation(&input, journal, publisher, plane, profiles)?;
                        let mut indexed = IndexedRetention {
                            plane,
                            route,
                            profile_digest: profiles.profile_digest(),
                        };
                        super::retention::consume_source_indexed(
                            input,
                            journal,
                            publisher,
                            config,
                            &mut indexed,
                        )
                    },
                )?;
                custody
                    .owner
                    .owner
                    .emit(writer, &output.line)
                    .map_err(|error| error.code().to_owned())?;
                if !output.current {
                    return Err("NATIVE_RETENTION_CURRENT_QUALIFICATION_REFUSED".into());
                }
                Ok(())
            },
        );
        match result {
            Ok(()) => Ok(()),
            Err(mut failure) => {
                failure.report_checked(writer);
                Err(failure.code().to_owned())
            }
        }
    }
}

#[allow(clippy::too_many_lines)] // Keep actual schema readback and its following adoption in one ordered scope.
fn ensure_generation(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    plane: &mut BlockingRealQueryPlane,
    profiles: &QualifiedNativeProfiles,
) -> Result<CollectionRoute, String> {
    let native = input.native;
    let work = |unit| native.work(unit);
    let mut intended =
        index_bootstrap::prepare_index_bootstrap(native, journal, publisher, profiles)
            .map_err(str::to_owned)?;
    // The bridge owns its literal vendor name. This stable locator recipe and
    // the retained genuine generation identify one route; it is never an ID alias.
    let route = CollectionRoute {
        generation: intended.generation(),
        physical_name: OpaqueId::new("native-raw-code-index-v1")
            .map_err(|_| "NATIVE_INDEX_ROUTE_INVALID")?,
    };
    let context = CanonicalRequestContext::with_deadline(
        input.deadline,
        Arc::new(native.context().cancellation().clone()),
    )
    .map_err(|error| error.code().to_owned())?;
    match intended.disposition() {
        IndexBootstrapDisposition::Prepared => {
            match plane.verify_indexed_generation(&route, intended.schema(), &context) {
                Err(BridgeError::CollectionNotFound) => {}
                Err(error) => return Err(error.code().to_owned()),
                Ok(()) => return Err("NATIVE_INDEX_PREEXISTING_GENERATION_REFUSED".into()),
            }
            index_bootstrap::mark_index_create_dispatched(
                native,
                journal,
                publisher,
                &mut intended,
            )
            .map_err(str::to_owned)?;
            // One attempt only. An uncertain create retains the dispatched row
            // and exact generation; the next invocation performs readback first.
            plane
                .create_indexed_generation(&route, intended.schema(), &context)
                .map_err(|error| error.code().to_owned())?;
            plane
                .verify_indexed_generation(&route, intended.schema(), &context)
                .map_err(|error| error.code().to_owned())?;
        }
        IndexBootstrapDisposition::Dispatched | IndexBootstrapDisposition::Indexed => {
            plane
                .verify_indexed_generation(&route, intended.schema(), &context)
                .map_err(|error| error.code().to_owned())?;
        }
        IndexBootstrapDisposition::OutcomeUnknown => {
            return Err("NATIVE_INDEX_BOOTSTRAP_OUTCOME_UNKNOWN".into());
        }
    }
    if intended.disposition() != IndexBootstrapDisposition::Indexed {
        input
            .security
            .checkpoint()
            .map_err(|error| error.to_string())?;
        if let Some(check) = input.current_source {
            check()?;
        }
        let registry = original_port(journal, publisher, input.deadline, &work)?
            .restore(native.context())
            .map_err(|_| "NATIVE_INDEX_SOURCE_SNAPSHOT_REFUSED")?
            .into_registry();
        let identity = journal.identity();
        let guards = input
            .security
            .publication_guards(
                identity.owner_epoch,
                registry.revision(),
                profiles.profile_digest(),
            )
            .map_err(|error| error.to_string())?;
        let state = PublicationVisibilityState {
            collection_generation_id: intended.generation(),
            schema_identity_digest: *intended
                .schema()
                .identity()
                .map_err(|_| "NATIVE_INDEX_SCHEMA_INVALID")?
                .as_digest(),
            visible_epoch: Epoch::new(0).map_err(|_| "NATIVE_INDEX_EPOCH_INVALID")?,
            guards,
            last_receipt: None,
        };
        let (operation, digest) =
            phase_id(&input.command.operation, "indexed-profile-adoption", &work)
                .map_err(|error| error.to_string())?;
        let mut port = journal
            .bind_control_port_with_original_context::<FreshRootCancellation, _>(
                publisher,
                input.deadline,
                &work,
            )
            .map_err(|error| error.control.code().to_owned())?;
        let adoption = port
            .prepare_indexed_profile_adoption_with_original_context(
                state,
                operation,
                digest,
                input.deadline,
                &work,
            )
            .map_err(|error| error.control.code().to_owned())?;
        if let Err(error) =
            port.adopt_indexed_profile_with_original_context(&adoption, input.deadline, &work)
            && !matches!(
                port.recover_indexed_profile_adoption_with_original_context(
                    &adoption,
                    input.deadline,
                    &work
                )
                .map_err(|_| "NATIVE_INDEX_ADOPTION_OUTCOME_UNKNOWN")?,
                CommitRecoveryDecision::Committed(_)
            )
        {
            return Err(error.control.code().to_owned());
        }
        published_metadata(journal, publisher, input.deadline, &work)?;
    }
    native
        .source_full_checkpoint()
        .map_err(|error| error.to_string())?;
    refresh_guard_image(input, journal, publisher, profiles.profile_digest())?;
    Ok(route)
}

/// Joins actual C03 and C18 observations in C02 before admission. This is a
/// guard image of those owners, not a new counter source or access authority.
pub(super) fn refresh_guard_image(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    profile_digest: search_contracts::Blake3Digest32,
) -> Result<(), String> {
    input
        .native
        .source_full_checkpoint()
        .map_err(|error| error.to_string())?;
    input
        .security
        .checkpoint()
        .map_err(|error| error.to_string())?;
    if let Some(check) = input.current_source {
        check()?;
    }
    let work = |unit| input.native.work(unit);
    let registry = journal
        .read_registry_snapshot_with_original_context(input.deadline, &work)
        .map_err(|error| error.control.code().to_owned())?;
    let (security, _) = journal
        .observe_security_restriction_with_original_context(
            input.namespace.ownership.source_namespace_id,
            input.deadline,
            &work,
        )
        .map_err(|error| error.control.code().to_owned())?;
    let identity = journal.identity();
    {
        let mut port = journal
            .bind_control_port_with_original_context::<FreshRootCancellation, _>(
                publisher,
                input.deadline,
                &work,
            )
            .map_err(|error| error.control.code().to_owned())?;
        let checkpoint = port
            .read_publication_checkpoint_with_original_context(input.deadline, &work)
            .map_err(|error| error.control.code().to_owned())?
            .ok_or("NATIVE_INDEX_CHECKPOINT_REQUIRED")?;
        let guards = input
            .security
            .publication_guards(
                identity.owner_epoch,
                registry.registry_revision(),
                profile_digest,
            )
            .map_err(|error| error.to_string())?;
        if guards == checkpoint.visibility().guards {
            return Ok(());
        }
        let phase = format!(
            "publication-guard-image:{}:{}",
            checkpoint.generation(),
            registry.registry_revision()
        );
        let (operation, digest) =
            phase_id(&input.command.operation, &phase, &work).map_err(|error| error.to_string())?;
        let request = port
            .prepare_publication_guard_image_with_original_context(
                &checkpoint,
                &registry,
                std::slice::from_ref(&security),
                guards,
                operation,
                digest,
                input.deadline,
                &work,
            )
            .map_err(|error| error.control.code().to_owned())?;
        let mutation = search_control_redb::control_mutation_identity(operation)
            .map_err(|_| "NATIVE_INDEX_GUARD_OPERATION_INVALID")?;
        if let Err(error) = port.transact_publication_guard_image_with_original_context(
            &request,
            &mutation,
            input.deadline,
            &work,
        ) && !matches!(
            port.recover_publication_guard_image_with_original_context(
                &request,
                &mutation,
                input.deadline,
                &work
            )
            .map_err(|_| "NATIVE_INDEX_GUARD_OUTCOME_UNKNOWN")?,
            CommitRecoveryDecision::Committed(_)
        ) {
            return Err(error.control.code().to_owned());
        }
    }
    published_metadata(journal, publisher, input.deadline, &work)?;
    input
        .native
        .source_full_checkpoint()
        .map_err(|error| error.to_string())?;
    input
        .security
        .checkpoint()
        .map_err(|error| error.to_string())?;
    Ok(())
}
