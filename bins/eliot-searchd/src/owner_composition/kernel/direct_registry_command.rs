//! Real normal caller for the sole fresh DIRECT database. No error fallback.
use super::{DirectRegistryOwner, NativeRootRequest, NativeWork, W};
use crate::config_composition::{self as config, RootLocatorHints};
use search_runtime_owner::{DataRootOpenMode, OwnerError};
use search_source_registry::{
    DEFAULT_REGISTRY_LIMITS, error::RegistryControlPort, recovery::RegistryBatchLookup,
};
use std::{
    ffi::{OsStr, OsString},
    process::ExitCode,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use std::{io::Write, path::Path, time::Instant};

pub fn maybe_run() -> Option<ExitCode> {
    let started = Instant::now();
    let arguments = std::env::args_os().skip(1).take(257).collect::<Vec<_>>();
    let Ok((selected, _)) = config::strip_config_args_os(&arguments) else {
        if !arguments.iter().any(|v| is_direct_command(v)) {
            return None;
        }
        return Some(ExitCode::from(2)); // Invalid full input never falls through to another owner.
    };
    if !is_direct_command(selected.first()?) {
        return None;
    }
    Some(
        match run(&arguments, started, &mut std::io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(2),
        },
    )
}

// Same closed selection on valid and malformed config inputs: no owner fallback.
fn is_direct_command(argument: &OsStr) -> bool {
    argument.to_str().is_some_and(|command| {
        matches!(
            command,
            "--register-source-root"
                | "--unregister-source-root"
                | "--source-roots"
                | "--health-data-root"
                | "--verify-root"
                | "--list-sources"
                | "--serve-data-root"
                | "--search-root"
                | "--search-root-ascii-insensitive"
                | "--read-revision"
                | "--sync-source-roots"
        )
    })
}

pub(super) fn run(
    arguments: &[OsString],
    started: Instant,
    writer: &mut impl Write,
) -> Result<(), String> {
    let (selected, cli) = config::strip_config_args_os(arguments)?;
    let command = selected
        .first()
        .and_then(|v| v.to_str())
        .ok_or("SOURCE_ROOT_USAGE_ERROR")?;
    let expected = match command {
        "--register-source-root"
        | "--unregister-source-root"
        | "--search-root"
        | "--search-root-ascii-insensitive" => 3,
        "--read-revision" => 5,
        _ => 2,
    };
    // Unsupported complete inputs are never silently truncated or ignored.
    if selected.len() != expected || !cli.overrides.is_empty() {
        return Err("DIRECT_INPUT_UNAVAILABLE".into());
    }
    let mode = if command == "--register-source-root" {
        DataRootOpenMode::OpenExistingMutating
    } else {
        DataRootOpenMode::InspectExistingReadOnly
    };
    let deadline = started
        .checked_add(Duration::from_secs(120))
        .ok_or("DIRECT_DEADLINE_INVALID")?;
    let limits = super::super::super::native_request::NativeRootLimits::new([
        256,
        256 * 1024,
        512 * 1024,
        1024 * 1024 * 1024,
        16384,
        256,
    ])
    .map_err(|e| e.code().to_owned())?;
    let (request, _) = NativeRootRequest::for_direct_registry(
        arguments,
        mode,
        deadline,
        Arc::new(AtomicBool::new(false)),
        limits,
    )
    .map_err(|e| e.code().to_owned())?;
    let mut work = NativeWork::new();
    let preflight = capture_hints(&request, &mut work, cli, command, &selected);
    let hints = match preflight {
        Ok(value) => value,
        Err(error) => {
            let line = format!(
                "{{\"error\":\"{}\",\"ready\":false}}",
                eliot_searchd::diagnostics::sanitize_code(&error)
            );
            request
                .emit_using_work(&mut work, writer, &line)
                .map_err(|e| e.code().to_owned())?;
            return Err(error);
        }
    };
    let owner = DirectRegistryOwner::open_for_command(request, work, writer)?;
    owner.execute(
        &hints,
        if command == "--register-source-root" {
            Some(Path::new(&selected[2]))
        } else {
            None
        },
        writer,
    )
}

pub(super) fn bootstrap_policy() -> Result<search_source_admission::ValidatedAdmissionConfig, String>
{
    use search_source_admission as a;
    let (platform, capabilities) = admission_environment();
    // Explicit closed bootstrap_v1 selection, all five compiled fields validated.
    // The partial W1 source_admission section is deliberately not a policy input.
    let config = a::validate_section(
        &a::compiled_defaults(),
        &platform,
        &capabilities,
    )
    .map_err(|_| "DIRECT_BOOTSTRAP_POLICY_UNAVAILABLE")?;
    a::compile_bootstrap_policy_v1(&config).map_err(|_| "DIRECT_BOOTSTRAP_POLICY_UNAVAILABLE")?;
    Ok(config)
}

fn admission_environment() -> (
    search_source_admission::SectionPlatform,
    search_source_admission::AcceptedCapabilities,
) {
    use search_source_admission as a;
    (
        a::SectionPlatform { os: "windows".into(), arch: "x64".into() },
        a::AcceptedCapabilities { profiles: vec![a::BASELINE_PROFILE.into()] },
    )
}

impl DirectRegistryOwner {
    pub(super) fn prepare_output(
        &mut self,
        hints: &RootLocatorHints,
        selected: Option<&Path>,
    ) -> Result<DirectOutput, String> {
        // Owned current Native epoch, distinct from the historical RootRecord epoch.
        let native_owner_epoch = self.owner.record.epoch;
        let deadline = self.owner.request.credential_context().0;
        let fresh_operation = original_invocation_operation(&self.owner.request)?;
        let mut read = super::super::read_child::direct_operation(&mut self.owner)
            .map_err(|e| e.code().to_owned())?;
        let native = read.fresh_root_context().map_err(|e| e.to_string())?;
        let callback = |work| native.work(work);
        let journal = self
            .control
            .journal
            .as_mut()
            .ok_or("DIRECT_JOURNAL_UNAVAILABLE")?;
        let mut port = original_port(journal, &mut self.control.publisher, deadline, &callback)?;
        let selected_id = selected.map(|path| hints.selected(path)).transpose()?;
        let (restored, original) = if selected_id.is_some() {
            port.restore_for_root(selected_id, native.context())
                .map_err(|_| "DIRECT_RESTORE_REFUSED")?
        } else {
            (
                port.restore(native.context())
                    .map_err(|_| "DIRECT_RESTORE_REFUSED")?,
                None,
            )
        };
        let mut registry = restored.into_registry();
        let operation = original.unwrap_or(fresh_operation);
        let policy = bootstrap_policy()?;
        let mut registered = false;
        let mut current = true;
        let mut historical_revision = None;
        if let Some(path) = selected {
            let id = hints.selected(path)?;
            // Always inspect immutable original operation before fresh O/Q preparation.
            match port
                .load_batch(&operation, native.context())
                .map_err(|_| "DIRECT_LOOKUP_REFUSED")?
            {
                RegistryBatchLookup::Committed(original) => {
                    let exact = original.record().deltas().iter().any(|v| matches!(v.after(),
                        search_source_registry::recovery::RegistryStateRecord::Root(root) if root.root_binding_id == id));
                    if !exact {
                        return Err("DIRECT_ORIGINAL_OPERATION_CONFLICT".into());
                    }
                    historical_revision = Some(original.record().registry_revision_after());
                    // Historical commit is separate from current policy/lifecycle.
                    // Requalification never reactivates a fenced RootRecord.
                    match native.requalify_existing(path, id, &registry, &policy) {
                        Ok(_qualified) => {}
                        Err(_) => current = false,
                    }
                }
                RegistryBatchLookup::Absent { .. } if registry.roots().contains_key(&id) => {
                    let _current = native
                        .requalify_existing(path, id, &registry, &policy)
                        .map_err(|e| e.to_string())?;
                }
                RegistryBatchLookup::Absent { revision } => {
                    let prepared = native
                        .prepare(path, id, operation, revision, &policy)
                        .map_err(|e| e.to_string())?;
                    let qualified = match prepared.register(&mut registry, &mut port) {
                        Ok(value) => value,
                        Err(error) => {
                            // Read back only the retained uncertain command. Never
                            // redispatch or turn recovery into fresh qualification.
                            if port.requires_recovery() {
                                let _original = port.recover_pending(native.context());
                            }
                            return Err(error.to_string());
                        }
                    };
                    // Producer has just completed full Native/readback qualification.
                    // Only bounded metadata is rendered; no source body/entropy follows.
                    if qualified.root().root_binding_id != id
                        || qualified.policy().revision() != qualified.root().policy_revision
                    {
                        return Err("DIRECT_ROOT_READBACK_REFUSED".into());
                    }
                    registered = true;
                }
            }
        } else {
            native
                .requalify_roots(&registry, hints, &policy)
                .map_err(|e| e.to_string())?;
        }
        native
            .work(W::Allocate(
                registry
                    .roots()
                    .len()
                    .saturating_mul(1024)
                    .saturating_add(4096),
            ))
            .map_err(|e| e.code().to_owned())?;
        drop(port);
        let owned_snapshot =
            published_metadata(journal, &mut self.control.publisher, deadline, &callback)?;
        let journal_generation = owned_snapshot.generation;
        Ok(DirectOutput::metadata(
            current,
            registered,
            historical_revision,
            &registry,
            journal_generation,
            native_owner_epoch,
        ))
    }
}

pub(super) struct DirectOutput {
    pub(super) line: String,
    pub(super) current: bool,
}

impl DirectOutput {
    // Observed input diagnostics only: no serialized value is a live read permit.
    fn metadata(
        current: bool,
        registered: bool,
        historical_revision: Option<u64>,
        registry: &search_source_registry::SourceRegistry,
        journal_generation: u64,
        native_owner_epoch: u64,
    ) -> Self {
        let (count, revision, roots) = registry_metadata(registry);
        let historical = historical_revision.is_some();
        let historical_revision =
            historical_revision.map_or_else(|| "null".into(), |v| v.to_string());
        Self {
            current,
            line: format!(
                "{{\"event\":\"direct_registry\",\"registered\":{registered},\"historical_commit\":{historical},\"historical_registry_revision\":{historical_revision},\"root_count\":{count},\"roots\":[{roots}],\"registry_revision\":{revision},\"journal_generation\":{journal_generation},\"native_owner_epoch\":{native_owner_epoch},\"current_qualification\":{current},\"qualification_scope\":\"root_metadata_only\",\"safe_read_security_barrier_revision\":null,\"source_read_refusal\":\"DIRECT_LIVE_SECURITY_OWNER_UNAVAILABLE\",\"ready\":false,\"source_body_serving\":false}}"
            ),
        }
    }
}

fn published_metadata(
    journal: &search_control_redb::PersistentControlJournal,
    publisher: &mut search_control_redb::ControlSnapshotPublisher,
    deadline: Instant,
    work: &impl Fn(W) -> Result<(), OwnerError>,
) -> Result<std::sync::Arc<search_control_redb::ControlSnapshot>, String> {
    journal
        .publish_direct_with_original_context(publisher, deadline, work)
        .map_err(|_| "DIRECT_SNAPSHOT_PUBLICATION_REFUSED")?;
    let snapshot = publisher.current().ok_or("DIRECT_SNAPSHOT_SUSPENDED")?;
    if snapshot.identity != journal.identity() || journal.requires_recovery() {
        return Err("DIRECT_SNAPSHOT_IDENTITY_REFUSED".into());
    }
    Ok(snapshot)
}

fn original_port<'a, C: search_ports::CancellationProbe>(
    journal: &'a mut search_control_redb::PersistentControlJournal,
    publisher: &'a mut search_control_redb::ControlSnapshotPublisher,
    deadline: Instant,
    work: &'a dyn Fn(W) -> Result<(), OwnerError>,
) -> Result<crate::registry_control::RegistryJournal<'a, C>, String> {
    let identity = journal.identity();
    let (platform, capabilities) = admission_environment();
    crate::registry_control::RegistryJournal::bind_original_v4(
        journal,
        publisher,
        identity,
        DEFAULT_REGISTRY_LIMITS,
        deadline,
        work,
        platform,
        capabilities,
    )
    .map_err(|_| "DIRECT_BIND_REFUSED".into())
}

// This is the invocation's stable operation encoding, not restored Root authority.
fn original_invocation_operation(
    request: &NativeRootRequest,
) -> Result<search_contracts::OpaqueId, String> {
    search_contracts::OpaqueId::new(format!(
        "native:{}",
        super::super::super::codec::hex(request.binding().0.as_bytes())
    ))
    .map_err(|_| "DIRECT_OPERATION_INVALID".into())
}

// All storage and Native qualification completes before this metadata-only render.
fn registry_metadata(registry: &search_source_registry::SourceRegistry) -> (usize, u64, String) {
    let roots = registry
        .roots()
        .values()
        .map(|root| {
            format!(
                "{{\"root_binding_id\":\"{}\",\"canonical_root_digest\":\"{}\",\"record_revision\":{},\"registry_revision\":{},\"policy_revision\":{},\"policy_fingerprint\":\"{}\"}}",
                super::super::super::codec::hex(root.root_binding_id.as_bytes()),
                root.canonical_root_digest,
                root.record_revision.get(),
                root.registry_revision,
                root.policy_revision.get(),
                root.policy_fingerprint,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    (registry.roots().len(), registry.revision(), roots)
}

fn capture_hints(
    request: &NativeRootRequest,
    work: &mut NativeWork,
    cli: config::CliConfigArgs,
    command: &str,
    selected: &[OsString],
) -> Result<RootLocatorHints, String> {
    let file = if let Some(path) = cli.config_file {
        work.open(request).map_err(|e| e.code().to_owned())?;
        work.read(request, config::MAX_CAPTURED_FILE_BYTES)
            .map_err(|e| e.code().to_owned())?;
        let bytes = work
            .call(request, || {
                config::read_config_file_bytes(&path)
                    .map_err(|_| OwnerError::OwnerOperationConflict)
            })
            .map_err(|e| e.code().to_owned())?;
        let document = config::capture_file_document(&bytes, "direct-root-locators")
            .map_err(|_| "DIRECT_CONFIG_INVALID")?;
        if document
            .declared_sections()
            .any(|v| v.as_str() != "source_root_locators")
        {
            return Err("DIRECT_CONFIG_VARIANT_UNAVAILABLE".into());
        }
        Some(document)
    } else {
        None
    };
    let effective = config::build_effective(file, None, None, "direct", "direct")
        .map_err(|_| "DIRECT_CONFIG_INVALID")?;
    let hints = RootLocatorHints::from_config(&effective)?;
    if command == "--register-source-root" {
        hints.selected(Path::new(&selected[2]))?;
    }
    if !matches!(
        command,
        "--register-source-root" | "--source-roots" | "--health-data-root" | "--verify-root"
    ) {
        return Err("DIRECT_CONSUMER_UNAVAILABLE".into());
    }
    Ok(hints)
}
