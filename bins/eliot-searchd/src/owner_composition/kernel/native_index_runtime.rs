//! Bounded Qdrant startup and shutdown around one ordinary Root operation.
//! The same Root, child, selected lease and bridge serve the supplied consumer.

use search_contracts::{self as c, CanonicalValue as V};
use search_os_secrets::SecretReference;
use search_os_secrets_windows::QdrantApiSecretLease;
use search_ports::OperationContext;
use search_qdrant_bridge::{
    BridgeError, BridgeLimits, QdrantApiKeyLease, QdrantApiKeyLeaseProvider,
    QdrantConnectionBinding, QdrantEndpointIdentity, QdrantLoopbackHost,
    live::LiveEndpoint,
    real::{BlockingRealQueryPlane, CanonicalRequestContext},
};
use search_qdrant_supervisor::{
    ContainmentEvidence, LaunchPlan, LoopbackEndpoint, LoopbackHost, OwnedChild, ProcessConfig,
    QdrantOwnerFence, SecretLeaseEvidence, SecretMaterial, validate_process_config,
};
use search_runtime_owner::OwnerError;
use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::super::read_child::{direct_operation, fresh_root::FreshRootCancellation};
use super::{DirectRegistryOwner, NativeProcessRootCustody, QualifiedNativeProfiles};
use crate::config_composition::EffectiveDaemonConfig;

const SHUTDOWN_TIMEOUT_MS: u64 = 5_000;

/// An actual selected C27 lease and its unchanged issuing context. Provisioning
/// remains in C27; this value cannot create a credential or authorize inventory.
pub(super) struct StartupSecret {
    pub(super) lease: QdrantApiSecretLease<FreshRootCancellation>,
    pub(super) context: OperationContext<FreshRootCancellation>,
}

struct LiveRoot(Arc<std::sync::atomic::AtomicBool>);
impl Drop for LiveRoot {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

struct ProcessLease {
    selected: Arc<StartupSecret>,
    child: Arc<Mutex<OwnedChild>>,
    live_root: Arc<std::sync::atomic::AtomicBool>,
    binding: QdrantConnectionBinding,
}

impl QdrantApiKeyLeaseProvider for ProcessLease {
    fn monotonic_now_ticks(&self) -> Result<u64, BridgeError> {
        self.selected
            .lease
            .with_secret_live_window(&self.selected.context, |_, _, _, observed, _| {
                observed.get()
            })
            .map_err(|_| BridgeError::AuthenticationLeaseExpired)
    }

    fn with_secret(
        &self,
        binding: &QdrantConnectionBinding,
        _: u64,
        callback: &mut dyn FnMut(&[u8]) -> Result<(), BridgeError>,
    ) -> Result<(), BridgeError> {
        if *binding != self.binding || !self.live_root.load(std::sync::atomic::Ordering::Acquire) {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        // Retain and query the actual Child; no process is opened by PID.
        let mut child = self
            .child
            .lock()
            .map_err(|_| BridgeError::SupervisorReceiptMismatch)?;
        if child.pid() != binding.process_id()
            || child
                .containment_observation()
                .and_then(|value| value.process_creation_marker())
                != Some(binding.creation_marker())
            || child.verified().sha256().as_bytes() != binding.artifact_digest().as_bytes()
            || !child.is_running()
        {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        self.selected
            .lease
            .with_secret_live_window(&self.selected.context, |bytes, reference, _, _, expires| {
                if reference != self.selected.lease.reference()
                    || expires != binding.secret_expires_at_tick()
                    || reference.binding().installation_incarnation_id()
                        != binding.installation_incarnation_id()
                {
                    return Err(BridgeError::AuthenticationInvalid);
                }
                callback(bytes)
            })
            .map_err(|_| BridgeError::AuthenticationLeaseExpired)?
    }
}

/// Runs one finite indexed command through the actual contained process and
/// operational bridge. It never reopens Root or advances its owner epoch.
pub(super) fn with_owned_indexed<T>(
    owner: DirectRegistryOwner,
    config: &EffectiveDaemonConfig,
    provision: impl FnOnce(
        &mut NativeProcessRootCustody,
        &EffectiveDaemonConfig,
    ) -> Result<StartupSecret, String>,
    consume: impl FnOnce(
        &mut NativeProcessRootCustody,
        &mut BlockingRealQueryPlane,
        &QualifiedNativeProfiles,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let startup_started = Instant::now();
    let settings = config.native_qdrant_settings();
    if !settings.enabled || config.selected_profile().as_str() != "lexical" {
        return Err("NATIVE_INDEXED_PROFILE_REQUIRED".into());
    }
    let mut custody = owner
        .into_process_custody(search_retention::sweep::SweepLimits::BASELINE)
        .map_err(|error| error.code().to_owned())?;
    let (artifact, artifact_pin) = custody
        .measure_qdrant_artifact(
            settings
                .artifact_path
                .as_deref()
                .ok_or("NATIVE_QDRANT_ARTIFACT_REQUIRED")?,
            settings
                .expected_sha256
                .ok_or("NATIVE_QDRANT_ARTIFACT_REQUIRED")?,
            settings
                .expected_version
                .as_ref()
                .ok_or("NATIVE_QDRANT_ARTIFACT_REQUIRED")?
                .as_str(),
        )
        .map_err(|error| error.code().to_owned())?;
    let (directory, data_path, user) = custody
        .prepare_qdrant_data_directory()
        .map_err(|error| error.code().to_owned())?;
    let (profiles, deadline, cancellation, facts) = {
        let mut operation =
            direct_operation(&mut custody.owner.owner).map_err(|error| error.code().to_owned())?;
        let native = operation
            .fresh_root_context()
            .map_err(|error| error.to_string())?;
        let profiles =
            QualifiedNativeProfiles::qualify_for_startup(&native).map_err(str::to_owned)?;
        (
            profiles,
            native.original_deadline(),
            native.context().cancellation().clone(),
            native
                .revision_dek_root_facts()
                .map_err(|error| error.to_string())?,
        )
    };
    let selected = Arc::new(provision(&mut custody, config)?);
    let startup_deadline = startup_started
        .checked_add(Duration::from_millis(settings.startup_timeout_ms))
        .ok_or("NATIVE_QDRANT_DEADLINE_INVALID")?
        .min(deadline);
    let identity = custody.journal_identity();
    let owner = QdrantOwnerFence {
        data_root_id: identity.data_root_id,
        installation_incarnation_id: identity.installation_incarnation_id,
        owner_epoch: identity.owner_epoch,
    };
    let (http, grpc) = select_ports(&mut custody, settings.port)?;
    // Reserve bounded plan/identity framing and the one secret copy before
    // entering the lease callback. No native work allowance is renewed here.
    {
        let root = &mut custody.owner.owner;
        root.work
            .read(
                &root.request,
                32 * 1024 + search_qdrant_supervisor::MAX_SECRET_BYTES,
            )
            .map_err(|error| error.code().to_owned())?;
    }
    let (plan, material) = selected
        .lease
        .with_secret_live_window(
            &selected.context,
            |bytes, reference, _, observed, expires| {
                if reference.binding().installation_id() != facts.installation
                    || reference.binding().installation_incarnation_id() != facts.incarnation
                    || reference.binding().user_scope_digest().as_bytes() != &user
                    || settings.api_secret_ref.as_ref().map(|value| value.as_str())
                        != Some(reference.id().as_str())
                {
                    return Err("NATIVE_QDRANT_SECRET_ROOT_MISMATCH");
                }
                let evidence = lease_evidence(reference, expires)?;
                let endpoint_digest = digest(
                    "eliot/cbor/native-qdrant-endpoint/v1",
                    vec![
                        V::U64(u64::from(http)),
                        V::U64(u64::from(grpc)),
                        blob(owner.data_root_id.as_bytes())?,
                    ],
                )?;
                let data_digest = directory.identity_digest();
                let config_digest = c::Blake3Digest32::from_bytes(*config.fingerprint().as_bytes());
                let qualified = validate_process_config(
                    ProcessConfig {
                        owner_fence: owner,
                        data_directory_digest: data_digest,
                        endpoint: LoopbackEndpoint {
                            endpoint_digest,
                            port: NonZeroU32::new(u32::from(http))
                                .ok_or("NATIVE_QDRANT_PORT_INVALID")?,
                        },
                        bind_is_loopback: true,
                        single_node: true,
                        startup_timeout_ticks: NonZeroU64::new(settings.startup_timeout_ms)
                            .ok_or("NATIVE_QDRANT_TIMEOUT_INVALID")?,
                        shutdown_timeout_ticks: NonZeroU64::new(SHUTDOWN_TIMEOUT_MS)
                            .ok_or("NATIVE_QDRANT_TIMEOUT_INVALID")?,
                        restart_window_ticks: NonZeroU64::new(settings.restart_window_ms)
                            .ok_or("NATIVE_QDRANT_TIMEOUT_INVALID")?,
                        max_restarts_per_window: usize::from(settings.restart_max_attempts),
                        config_digest,
                    },
                    &artifact,
                    evidence,
                    observed,
                )
                .map_err(|error| error.code())?;
                let remaining = startup_deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("NATIVE_QDRANT_DEADLINE_EXPIRED")?;
                let startup_timeout =
                    Duration::from_millis(settings.startup_timeout_ms).min(remaining);
                let operation = c::OpaqueId::new(format!(
                    "native-qdrant:{}",
                    super::super::super::codec::hex(selected.context.request_id().as_bytes())
                ))
                .map_err(|_| "NATIVE_QDRANT_OPERATION_INVALID")?;
                let plan = LaunchPlan::new(
                    operation,
                    owner,
                    artifact.clone(),
                    qualified,
                    evidence,
                    evidence.purpose_digest,
                    settings
                        .artifact_path
                        .clone()
                        .ok_or("NATIVE_QDRANT_ARTIFACT_REQUIRED")?,
                    search_qdrant_supervisor::QUALIFIED_EXE_BYTES,
                    data_path.clone(),
                    LoopbackHost::V4,
                    http,
                    grpc,
                    startup_timeout,
                    Duration::from_millis(SHUTDOWN_TIMEOUT_MS),
                    ContainmentEvidence::windows_containment_required(),
                    observed,
                )
                .map_err(|error| error.code())?;
                let material =
                    SecretMaterial::from_bytes(bytes.to_vec()).map_err(|error| error.code())?;
                Ok::<_, &'static str>((plan, material))
            },
        )
        .map_err(|error| error.code().to_owned())?
        .map_err(str::to_owned)?;
    // The spawn outcome is outside the C27 callback: a late TTL refusal cannot
    // discard Child/Job custody returned by the actual native producer.
    let expected_lease = *plan.secret_lease();
    let mut lease_check = |expected: &SecretLeaseEvidence| {
        selected
            .lease
            .with_secret_live_window(&selected.context, |_, reference, _, _, expires| {
                if reference == selected.lease.reference()
                    && *expected == expected_lease
                    && expires == expected_lease.expires_at_tick
                {
                    Ok(())
                } else {
                    Err(OwnerError::OwnerGuardMismatch)
                }
            })
            .map_err(|_| OwnerError::OwnerDeadlineInvalid)?
    };
    let spawned = custody
        .spawn_qdrant_child(&plan, &material, &directory, user, &mut lease_check)
        .map_err(|_| "NATIVE_QDRANT_STARTUP_REFUSED".to_owned())?;
    drop(material);
    let child = Arc::new(Mutex::new(OwnedChild::from(spawned)));
    let live = LiveRoot(Arc::new(std::sync::atomic::AtomicBool::new(true)));
    let binding = connection_binding(&plan, &child)?;
    let provider = ProcessLease {
        selected,
        child: Arc::clone(&child),
        live_root: Arc::clone(&live.0),
        binding,
    };
    let endpoint = LiveEndpoint::loopback("127.0.0.1", http, grpc)
        .map_err(|_| "NATIVE_QDRANT_ENDPOINT_INVALID")?;
    let context = CanonicalRequestContext::with_deadline(startup_deadline, Arc::new(cancellation))
        .map_err(|error| error.code().to_owned())?;
    let (mut plane, _report) = BlockingRealQueryPlane::connect_and_probe(
        &endpoint,
        binding,
        QdrantApiKeyLease::new(binding, provider),
        context,
        BridgeLimits::BASELINE,
        Duration::from_millis(settings.health_timeout_ms),
    )
    .map_err(|error| error.error().code().to_owned())?;
    let result = consume(&mut custody, &mut plane, &profiles);
    drop(plane);
    let stopped = child
        .lock()
        .map_err(|_| "NATIVE_QDRANT_CUSTODY_POISONED")?
        .terminate_bounded(deadline)
        .map_err(|_| "NATIVE_QDRANT_SHUTDOWN_UNKNOWN");
    drop(live);
    drop(child);
    drop((directory, artifact_pin));
    stopped?;
    let value = result?;
    custody
        .owner
        .finish()
        .map_err(|error| error.code().to_owned())?;
    Ok(value)
}

fn select_ports(
    custody: &mut NativeProcessRootCustody,
    selected: u16,
) -> Result<(u16, u16), String> {
    let root = &mut custody.owner.owner;
    let mut free_port = || {
        let listener = root.work.call(&root.request, || {
            std::net::TcpListener::bind("127.0.0.1:0")
                .map_err(|_| OwnerError::OwnerOperationConflict)
        })?;
        root.work.call(&root.request, || {
            listener
                .local_addr()
                .map(|address| address.port())
                .map_err(|_| OwnerError::OwnerOperationConflict)
        })
    };
    let ports = (
        free_port().map_err(|error| error.code().to_owned())?,
        free_port().map_err(|error| error.code().to_owned())?,
    );
    if ports.0 == 0 || ports.1 == 0 || ports.0 == ports.1 {
        return Err("NATIVE_QDRANT_PORT_CONFLICT".into());
    }
    if selected == 0 {
        Ok(ports)
    } else if selected == ports.1 {
        Err("NATIVE_QDRANT_PORT_CONFLICT".into())
    } else {
        Ok((selected, ports.1))
    }
}

fn connection_binding(
    plan: &LaunchPlan,
    child: &Mutex<OwnedChild>,
) -> Result<QdrantConnectionBinding, String> {
    let child = child.lock().map_err(|_| "NATIVE_QDRANT_CUSTODY_POISONED")?;
    let creation = child
        .containment_observation()
        .and_then(|value| value.process_creation_marker())
        .ok_or("NATIVE_QDRANT_CREATION_IDENTITY_MISSING")?;
    let owner = plan.owner_fence();
    let secret = plan.secret_lease();
    QdrantConnectionBinding::new(
        child.pid(),
        creation,
        owner.data_root_id,
        owner.installation_incarnation_id,
        owner.owner_epoch,
        plan.artifact().candidate().artifact_digest,
        QdrantEndpointIdentity::new(
            QdrantLoopbackHost::Ipv4,
            NonZeroU16::new(plan.http_port()).ok_or("NATIVE_QDRANT_PORT_INVALID")?,
            NonZeroU16::new(plan.grpc_port()).ok_or("NATIVE_QDRANT_PORT_INVALID")?,
            plan.config().config().endpoint.endpoint_digest,
        ),
        secret.secret_reference_digest,
        secret.installation_incarnation_id,
        secret.purpose_digest,
        secret.expires_at_tick,
    )
    .map_err(|error| error.code().to_owned())
}

fn lease_evidence(
    reference: &SecretReference,
    expires: NonZeroU64,
) -> Result<SecretLeaseEvidence, &'static str> {
    Ok(SecretLeaseEvidence {
        secret_reference_digest: secret_reference_digest(reference)?,
        installation_incarnation_id: reference.binding().installation_incarnation_id(),
        purpose_digest: digest(
            "eliot/cbor/native-qdrant-secret-purpose/v1",
            vec![text(reference.binding().purpose().as_str())?],
        )?,
        expires_at_tick: expires,
    })
}

fn secret_reference_digest(reference: &SecretReference) -> Result<c::Blake3Digest32, &'static str> {
    digest(
        "eliot/cbor/native-qdrant-secret-reference/v1",
        vec![
            text(reference.id().as_str())?,
            blob(reference.binding().installation_id().as_bytes())?,
            blob(reference.binding().installation_incarnation_id().as_bytes())?,
            blob(reference.binding().user_scope_digest().as_bytes())?,
            text(reference.binding().purpose().as_str())?,
            V::U64(reference.version().get()),
        ],
    )
}
fn text(value: &str) -> Result<V, &'static str> {
    c::CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")
}
fn blob(value: &[u8]) -> Result<V, &'static str> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")
}
fn digest(domain: &str, fields: Vec<V>) -> Result<c::Blake3Digest32, &'static str> {
    let value =
        V::Array(c::BoundedList::new(fields).map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")?);
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(domain).map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")?,
        &value,
        c::DigestInputLimit::new(4096).map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")?,
    )
    .map_err(|_| "NATIVE_QDRANT_IDENTITY_INVALID")
}
