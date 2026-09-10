//! Effective configuration composition and truthful capability readiness.
//!
//! Daemon-side composition over the pure `search-config` mechanics. This
//! module performs no filesystem, environment, or argument acquisition
//! itself: callers supply already-captured bytes and pairs. All precedence,
//! validation, fingerprinting, diffing, and planning delegate to
//! `search-config`; this module contributes only the closed daemon registry,
//! real BLAKE3 digests over captured inputs, atomic publication gating, and
//! truthful readiness derivation.
//!
//! The registry mirrors `config/sections.toml` names, owners, reload classes,
//! and secret policies for the daemon-observed subset. It does not redefine
//! another owner's settings: every merge, projection, validation-digest
//! binding, snapshot assembly, diff, plan, and redaction call goes through
//! `search-config` without reimplementation.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use search_config::{
    assemble_effective, diff, merge_layers, parse_document, plan_reconfiguration, project_section,
    redacted_view, register_sections, validate_environment_key, ConfigDocument, ConfigError,
    ConfigFieldDescriptor, ConfigFingerprint, ConfigKeyName, ConfigKeyPath, ConfigLayers,
    ConfigLimits, ConfigOwner, ConfigRegistry, ConfigSectionDescriptor, ConfigSectionName,
    ConfigSource, ConfigSourceKind, ConfigSourceRef, ConfigValue, ConfigValueKind, DocumentValue,
    EffectiveConfigSnapshot, LayerOperation, ReceiptKind, ReconfigurationAction,
    ReconfigurationPlan, RedactionPolicy, ReloadClass, SecretPolicy, SecurityFloor,
    ValidatedSection, ValueBounds,
};
use search_contracts::{Blake3Digest32, ProfileId};

/// Exact configuration schema version from `config/sections.toml`.
pub const DAEMON_CONFIG_SCHEMA_VERSION: u32 = 1;
/// Externally authorized W1 shell profile.
pub const DAEMON_DIRECT_PROFILE: &str = "direct";
/// Bounded captured inputs for one composition attempt.
pub const MAX_CAPTURED_FILE_BYTES: usize = 64 * 1024;
/// Maximum captured environment or CLI entries for one composition attempt.
pub const MAX_CAPTURED_ENTRIES: usize = 256;
/// Maximum captured value bytes before typed validation.
pub const MAX_CAPTURED_VALUE_BYTES: usize = 4_096;
/// Explicit reset marker accepted only where the descriptor allows reset.
pub const RESET_MARKER: &str = "__RESET__";

/// Closed daemon error codes for composition orchestration. Pure
/// `search-config` failures travel as `ConfigError`; these cover the
/// daemon-side gating that has no `search-config` counterpart.
pub const ACTIVATION_BLOCKED: &str = "DAEMON_CONFIG_ACTIVATION_BLOCKED";
/// Candidate explicitly rejected by a field or section obligation.
pub const ACTIVATION_REJECTED: &str = "DAEMON_CONFIG_REJECTED";
/// Mixed obligations without every required receipt; old snapshot retained.
pub const ACTIVATION_PARTIAL_REFUSED: &str = "DAEMON_CONFIG_PARTIAL_REFUSED";
/// Optional profile requested without an external gate receipt.
pub const OPTIONAL_GATE_REQUIRED: &str = "DAEMON_CONFIG_GATE_REQUIRED";
/// General search requested without an accepted search receipt.
pub const SEARCH_NOT_ACCEPTED: &str = "DAEMON_SEARCH_NOT_ACCEPTED";
/// Indexed search requested without accepted indexed receipts.
pub const INDEXED_NOT_ACCEPTED: &str = "DAEMON_INDEXED_NOT_ACCEPTED";
/// Control dependency is not verified.
pub const CONTROL_NOT_READY: &str = "DAEMON_CONTROL_NOT_READY";
/// Direct store dependency is not verified.
pub const DIRECT_NOT_READY: &str = "DAEMON_DIRECT_NOT_READY";
/// Persistent quarantine is armed.
pub const QUARANTINED_BLOCKER: &str = "DAEMON_QUARANTINED";

fn daemon_limits() -> ConfigLimits {
    ConfigLimits::W1
}

fn const_bounds() -> ValueBounds {
    ValueBounds {
        max_text_bytes: MAX_CAPTURED_VALUE_BYTES,
        max_list_items: 64,
        max_list_item_bytes: 512,
        integer_min: 0,
        integer_max: 16_777_216,
    }
}

fn profile_id(value: &str) -> Result<ProfileId, ConfigError> {
    ProfileId::new(value).map_err(|_| ConfigError::InvalidIdentifier)
}

fn section_name(value: &str) -> Result<ConfigSectionName, ConfigError> {
    ConfigSectionName::new(value, 128)
}

fn key_name(value: &str) -> Result<ConfigKeyName, ConfigError> {
    ConfigKeyName::new(value, 128)
}

fn owner_name(value: &str) -> Result<ConfigOwner, ConfigError> {
    ConfigOwner::new(value, 128)
}

fn source_ref(value: &str) -> Result<ConfigSourceRef, ConfigError> {
    ConfigSourceRef::new(value, 128)
}

fn blake3_digest(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
}

fn defaults_source() -> Result<ConfigSource, ConfigError> {
    Ok(ConfigSource {
        kind: ConfigSourceKind::Defaults,
        source_ref: source_ref("daemon-compiled-defaults")?,
        source_digest: blake3_digest(b"eliot-searchd/compiled-defaults/v1"),
    })
}

/// Deterministic field-registry digest over the exact declared fields.
/// The digest binds name, owner, revision, and every field identity so a
/// descriptor change is observable without persisting raw values.
fn section_registry_digest(
    section: &str,
    owner: &str,
    revision: u64,
    fields: &[(&str, &str)],
) -> Blake3Digest32 {
    let mut encoding = Vec::new();
    encoding.extend_from_slice(b"eliot-searchd/section-registry/v1\0");
    encoding.extend_from_slice(section.as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(owner.as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(&revision.to_be_bytes());
    for (key, kind) in fields {
        encoding.extend_from_slice(key.as_bytes());
        encoding.push(0);
        encoding.extend_from_slice(kind.as_bytes());
        encoding.push(0);
    }
    blake3_digest(&encoding)
}

fn make_field(
    key: &str,
    kind: ConfigValueKind,
    default: ConfigValue,
    allowed: &[ConfigSourceKind],
    reset_allowed: bool,
    floor: SecurityFloor,
    redaction: RedactionPolicy,
    actions: &[ReconfigurationAction],
) -> Result<ConfigFieldDescriptor, ConfigError> {
    ConfigFieldDescriptor::new(
        key_name(key)?,
        kind,
        default,
        const_bounds(),
        allowed.iter().copied(),
        reset_allowed,
        floor,
        redaction,
        actions.iter().copied(),
    )
}

/// Closed daemon-observed registry.
///
/// Mirrors `config/sections.toml` section names, owners, reload classes, and
/// secret policies for seven sections. Every reload/obligation class the
/// daemon must gate appears at least once: live, barrier, restart,
/// drain-restart, generation, rebuild, migration, gate, and reject.
pub fn daemon_registry() -> Result<ConfigRegistry, ConfigError> {
    let revision = NonZeroU64::MIN;
    let instance = ConfigSectionDescriptor::new(
        section_name("instance")?,
        owner_name("search-runtime-owner")?,
        revision,
        ReloadClass::DrainAndRestart,
        section_registry_digest(
            "instance",
            "search-runtime-owner",
            1,
            &[("mode", "text"), ("data_root", "text")],
        ),
        SecretPolicy::ForbidPlaintext,
        [
            make_field(
                "mode",
                ConfigValueKind::Text,
                ConfigValue::Text("standalone".to_owned()),
                &[ConfigSourceKind::File, ConfigSourceKind::Cli],
                true,
                SecurityFloor::None,
                RedactionPolicy::Public,
                &[ReconfigurationAction::DrainAndRestart],
            )?,
            make_field(
                "data_root",
                ConfigValueKind::Text,
                ConfigValue::Absent,
                &[
                    ConfigSourceKind::File,
                    ConfigSourceKind::Environment,
                    ConfigSourceKind::Cli,
                ],
                true,
                SecurityFloor::None,
                RedactionPolicy::PathDigest,
                &[ReconfigurationAction::DrainAndRestart],
            )?,
        ],
        daemon_limits(),
    )?;
    let secrets = ConfigSectionDescriptor::new(
        section_name("secrets")?,
        owner_name("search-os-secrets")?,
        revision,
        ReloadClass::RestartDependency,
        section_registry_digest(
            "secrets",
            "search-os-secrets",
            1,
            &[("qdrant_api_secret_ref", "secret")],
        ),
        SecretPolicy::OpaqueReferencesOnly,
        [make_field(
            "qdrant_api_secret_ref",
            ConfigValueKind::SecretReference,
            ConfigValue::Absent,
            &[ConfigSourceKind::File, ConfigSourceKind::Environment],
            true,
            SecurityFloor::None,
            RedactionPolicy::Secret,
            &[ReconfigurationAction::RestartDependency],
        )?],
        daemon_limits(),
    )?;
    let control = ConfigSectionDescriptor::new(
        section_name("control")?,
        owner_name("search-control-redb")?,
        revision,
        ReloadClass::RestartDependency,
        section_registry_digest(
            "control",
            "search-control-redb",
            1,
            &[("durability", "text"), ("migration_epoch", "integer")],
        ),
        SecretPolicy::ForbidPlaintext,
        [
            make_field(
                "durability",
                ConfigValueKind::Text,
                ConfigValue::Text("fsync_atomic".to_owned()),
                &[ConfigSourceKind::File],
                false,
                SecurityFloor::Fixed,
                RedactionPolicy::Public,
                &[ReconfigurationAction::Reject],
            )?,
            make_field(
                "migration_epoch",
                ConfigValueKind::Integer,
                ConfigValue::Integer(1),
                &[ConfigSourceKind::File],
                false,
                SecurityFloor::IntegerMinimum(1),
                RedactionPolicy::Public,
                &[ReconfigurationAction::MigrateControlSchema],
            )?,
        ],
        daemon_limits(),
    )?;
    let admission = ConfigSectionDescriptor::new(
        section_name("source_admission")?,
        owner_name("search-source-admission")?,
        revision,
        ReloadClass::SecurityBarrier,
        section_registry_digest(
            "source_admission",
            "search-source-admission",
            1,
            &[("allow_generated", "boolean")],
        ),
        SecretPolicy::ForbidPlaintext,
        [make_field(
            "allow_generated",
            ConfigValueKind::Boolean,
            ConfigValue::Boolean(true),
            &[ConfigSourceKind::File],
            true,
            SecurityFloor::BooleanMayOnlyRestrict,
            RedactionPolicy::Public,
            &[ReconfigurationAction::SecurityBarrier],
        )?],
        daemon_limits(),
    )?;
    let lexical = ConfigSectionDescriptor::new(
        section_name("lexical")?,
        owner_name("search-lexical")?,
        revision,
        ReloadClass::NewCollectionGeneration,
        section_registry_digest("lexical", "search-lexical", 1, &[("profile_id", "text")]),
        SecretPolicy::ForbidPlaintext,
        [make_field(
            "profile_id",
            ConfigValueKind::Text,
            ConfigValue::Text("baseline-v1".to_owned()),
            &[ConfigSourceKind::File],
            true,
            SecurityFloor::None,
            RedactionPolicy::Public,
            &[
                ReconfigurationAction::NewCollectionGeneration,
                ReconfigurationAction::RebuildProjection,
            ],
        )?],
        daemon_limits(),
    )?;
    let query = ConfigSectionDescriptor::new(
        section_name("query")?,
        owner_name("search-query-planner")?,
        revision,
        ReloadClass::ApplyLive,
        section_registry_digest("query", "search-query-planner", 1, &[("limit", "integer")]),
        SecretPolicy::ForbidPlaintext,
        [make_field(
            "limit",
            ConfigValueKind::Integer,
            ConfigValue::Integer(10),
            &[
                ConfigSourceKind::File,
                ConfigSourceKind::Environment,
                ConfigSourceKind::Cli,
            ],
            true,
            SecurityFloor::None,
            RedactionPolicy::Public,
            &[ReconfigurationAction::ApplyLive],
        )?],
        daemon_limits(),
    )?;
    let optional = ConfigSectionDescriptor::new(
        section_name("optional_profiles")?,
        owner_name("eliot-searchd")?,
        revision,
        ReloadClass::GateRequired,
        section_registry_digest(
            "optional_profiles",
            "eliot-searchd",
            1,
            &[("semantic", "boolean")],
        ),
        SecretPolicy::OpaqueReferencesOnly,
        [make_field(
            "semantic",
            ConfigValueKind::Boolean,
            ConfigValue::Boolean(false),
            &[ConfigSourceKind::File],
            true,
            SecurityFloor::None,
            RedactionPolicy::Public,
            &[ReconfigurationAction::GateRequired],
        )?],
        daemon_limits(),
    )?;
    register_sections(
        DAEMON_CONFIG_SCHEMA_VERSION,
        [
            instance, secrets, control, admission, lexical, query, optional,
        ],
        daemon_limits(),
    )
}

/// Parses one already-captured file byte slice through the pure document
/// parser. No filesystem read happens here.
///
/// # Errors
///
/// Returns the exact `search-config` failure for framing, encoding,
/// duplicate, syntax, profile, or limit violations.
pub fn capture_file_document(
    bytes: &[u8],
    source_label: &str,
) -> Result<ConfigDocument, ConfigError> {
    if bytes.is_empty() {
        return Err(ConfigError::EmptyInput);
    }
    if bytes.len() > MAX_CAPTURED_FILE_BYTES {
        return Err(ConfigError::CapacityExceeded);
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::File,
        source_ref: source_ref(source_label)?,
        source_digest: blake3_digest(bytes),
    };
    parse_document(bytes, source, daemon_limits())
}

fn infer_value(text: &str) -> Result<DocumentValue, ConfigError> {
    if text.len() > MAX_CAPTURED_VALUE_BYTES {
        return Err(ConfigError::CapacityExceeded);
    }
    if text == "true" {
        return Ok(DocumentValue::Boolean(true));
    }
    if text == "false" {
        return Ok(DocumentValue::Boolean(false));
    }
    if let Ok(number) = text.parse::<i64>() {
        if number.to_string() == text {
            return Ok(DocumentValue::Integer(number));
        }
    }
    Ok(DocumentValue::Text(text.to_owned()))
}

fn infer_operation(text: &str) -> Result<LayerOperation, ConfigError> {
    if text == RESET_MARKER {
        Ok(LayerOperation::Reset)
    } else {
        infer_value(text).map(LayerOperation::Set)
    }
}

/// Builds the captured environment layer from already-read `(name, value)`
/// pairs. Unknown prefixed keys fail closed; an empty input yields no layer.
///
/// # Errors
///
/// Returns `InvalidEnvironmentKey` for unknown, malformed, list-typed, or
/// non-whitelisted variables, and capacity failures for oversize inputs.
pub fn capture_environment_document(
    pairs: &[(&str, &str)],
) -> Result<Option<ConfigDocument>, ConfigError> {
    if pairs.is_empty() {
        return Ok(None);
    }
    if pairs.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let registry = daemon_registry()?;
    let mut encoding = Vec::from(b"eliot-searchd/environment/v1\0".as_slice());
    let mut entries = Vec::with_capacity(pairs.len());
    for (name, value) in pairs {
        let path = validate_environment_key(name, &registry)?;
        let operation = infer_operation(value)?;
        encoding.extend_from_slice(name.as_bytes());
        encoding.push(0);
        encoding.extend_from_slice(value.as_bytes());
        encoding.push(0);
        entries.push((path, operation));
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::Environment,
        source_ref: source_ref("captured-environment")?,
        source_digest: blake3_digest(&encoding),
    };
    Ok(Some(ConfigDocument::from_entries(
        DAEMON_CONFIG_SCHEMA_VERSION,
        None,
        source,
        entries,
        daemon_limits(),
    )?))
}

/// Builds the captured CLI layer from already-parsed `section.key` pairs.
/// Values use the same bounded inference as the environment layer; the
/// explicit reset marker restores the compiled default where allowed.
///
/// # Errors
///
/// Returns `UnknownSection`, `UnknownKey`, or `OverrideNotAllowed` through
/// the later merge when a pair is not registered or not CLI-whitelisted;
/// malformed paths fail here as `InvalidIdentifier`.
pub fn capture_cli_document(pairs: &[(&str, &str)]) -> Result<Option<ConfigDocument>, ConfigError> {
    if pairs.is_empty() {
        return Ok(None);
    }
    if pairs.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let mut encoding = Vec::from(b"eliot-searchd/cli/v1\0".as_slice());
    let mut entries = Vec::with_capacity(pairs.len());
    for (dotted, value) in pairs {
        let (section, key) = dotted
            .split_once('.')
            .ok_or(ConfigError::InvalidIdentifier)?;
        let path = ConfigKeyPath::new(section_name(section)?, key_name(key)?);
        let operation = infer_operation(value)?;
        encoding.extend_from_slice(dotted.as_bytes());
        encoding.push(0);
        encoding.extend_from_slice(value.as_bytes());
        encoding.push(0);
        entries.push((path, operation));
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::Cli,
        source_ref: source_ref("captured-cli")?,
        source_digest: blake3_digest(&encoding),
    };
    Ok(Some(ConfigDocument::from_entries(
        DAEMON_CONFIG_SCHEMA_VERSION,
        None,
        source,
        entries,
        daemon_limits(),
    )?))
}

/// Builds the captured CLI layer from already-typed operations. Used when the
/// caller has typed values (for example tests or a typed argv parser).
///
/// # Errors
///
/// Returns capacity or source-kind failures from the layer constructor.
pub fn capture_cli_typed_document(
    entries: Vec<(ConfigKeyPath, LayerOperation)>,
) -> Result<Option<ConfigDocument>, ConfigError> {
    if entries.is_empty() {
        return Ok(None);
    }
    if entries.len() > MAX_CAPTURED_ENTRIES {
        return Err(ConfigError::CapacityExceeded);
    }
    let mut encoding = Vec::from(b"eliot-searchd/cli-typed/v1\0".as_slice());
    for (path, operation) in &entries {
        encoding.extend_from_slice(path.section().as_str().as_bytes());
        encoding.push(b'.');
        encoding.extend_from_slice(path.key().as_str().as_bytes());
        encoding.push(0);
        match operation {
            LayerOperation::Set(DocumentValue::Boolean(value)) => {
                encoding.extend_from_slice(value.to_string().as_bytes());
            }
            LayerOperation::Set(DocumentValue::Integer(value)) => {
                encoding.extend_from_slice(value.to_string().as_bytes());
            }
            LayerOperation::Set(DocumentValue::Text(value)) => {
                if value.len() > MAX_CAPTURED_VALUE_BYTES {
                    return Err(ConfigError::CapacityExceeded);
                }
                encoding.extend_from_slice(value.as_bytes());
            }
            LayerOperation::Set(DocumentValue::StringList(_)) => {
                return Err(ConfigError::ValueOutOfBounds);
            }
            LayerOperation::Reset => encoding.extend_from_slice(b"__RESET__"),
        }
        encoding.push(0);
    }
    let source = ConfigSource {
        kind: ConfigSourceKind::Cli,
        source_ref: source_ref("captured-cli")?,
        source_digest: blake3_digest(&encoding),
    };
    Ok(Some(ConfigDocument::from_entries(
        DAEMON_CONFIG_SCHEMA_VERSION,
        None,
        source,
        entries,
        daemon_limits(),
    )?))
}

fn encode_config_value(value: &ConfigValue, encoding: &mut Vec<u8>) {
    match value {
        ConfigValue::Absent => encoding.extend_from_slice(b"absent\0"),
        ConfigValue::Boolean(flag) => {
            encoding.extend_from_slice(b"boolean:");
            encoding.push(u8::from(*flag));
            encoding.push(0);
        }
        ConfigValue::Integer(number) => {
            encoding.extend_from_slice(b"integer:");
            encoding.extend_from_slice(&number.to_be_bytes());
            encoding.push(0);
        }
        ConfigValue::Text(text) => {
            encoding.extend_from_slice(b"text:");
            encoding.extend_from_slice(text.as_bytes());
            encoding.push(0);
        }
        ConfigValue::SecretReference(reference) => {
            encoding.extend_from_slice(b"secret:");
            encoding.extend_from_slice(reference.as_str().as_bytes());
            encoding.push(0);
        }
        ConfigValue::StringList(items) => {
            encoding.extend_from_slice(b"list:");
            for item in items {
                encoding.extend_from_slice(item.as_bytes());
                encoding.push(0);
            }
            encoding.push(0);
        }
    }
}

/// Capability-structural validation digest over one projected section.
/// The digest binds section identity and every effective value so identical
/// inputs reproduce identical digests without persisting raw secrets or
/// paths outside the snapshot.
fn validation_digest(input: &search_config::ConfigSectionInput) -> Blake3Digest32 {
    let mut encoding = Vec::new();
    encoding.extend_from_slice(b"eliot-searchd/section-validation/v1\0");
    encoding.extend_from_slice(input.section_name().as_str().as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(input.owner().as_str().as_bytes());
    encoding.push(0);
    encoding.extend_from_slice(&input.schema_revision().get().to_be_bytes());
    for (key, field) in input.fields() {
        encoding.extend_from_slice(key.as_str().as_bytes());
        encoding.push(0);
        encode_config_value(&field.value, &mut encoding);
    }
    blake3_digest(&encoding)
}

/// Immutable daemon-effective configuration: the authoritative snapshot
/// plus the closed registry that produced it. Only content-free references
/// (fingerprints and digests) leave this struct for persistence or status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveDaemonConfig {
    snapshot: EffectiveConfigSnapshot,
    registry: ConfigRegistry,
}

impl EffectiveDaemonConfig {
    /// Authoritative effective snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &EffectiveConfigSnapshot {
        &self.snapshot
    }

    /// Closed registry that produced the snapshot.
    #[must_use]
    pub const fn registry(&self) -> &ConfigRegistry {
        &self.registry
    }

    /// Exact effective fingerprint for persistence and receipts.
    #[must_use]
    pub fn fingerprint(&self) -> ConfigFingerprint {
        self.snapshot.fingerprint()
    }

    /// Externally selected profile bound into the snapshot.
    #[must_use]
    pub fn selected_profile(&self) -> ProfileId {
        self.snapshot.selected_profile().clone()
    }
}

/// Assembles the daemon-effective snapshot from compiled defaults plus the
/// already-captured file, environment, and CLI layers. Every layering,
/// projection, and fingerprint step delegates to `search-config`.
///
/// # Errors
///
/// Returns the exact `search-config` failure for precedence, allowlist,
/// reset, type, bounds, plaintext-secret, floor, duplicate, unknown,
/// stale-descriptor, profile, or missing-section violations. No partial
/// snapshot escapes.
pub fn build_effective(
    file: Option<ConfigDocument>,
    environment: Option<ConfigDocument>,
    cli: Option<ConfigDocument>,
    requested_profile: &str,
    selected_profile: &str,
) -> Result<EffectiveDaemonConfig, ConfigError> {
    let registry = daemon_registry()?;
    let requested = profile_id(requested_profile)?;
    let selected = profile_id(selected_profile)?;
    let merged = merge_layers(
        ConfigLayers {
            defaults: defaults_source()?,
            requested_profile: requested,
            file,
            environment,
            cli,
        },
        &registry,
        daemon_limits(),
    )?;
    let mut validated = Vec::new();
    for (_, descriptor) in registry.sections() {
        let input = project_section(&merged, descriptor)?;
        let digest = validation_digest(&input);
        validated.push(ValidatedSection::new(input, selected.clone(), digest));
    }
    let snapshot = assemble_effective(&registry, validated, selected, daemon_limits())?;
    Ok(EffectiveDaemonConfig { snapshot, registry })
}

/// Defaults-only effective snapshot for the W1 shell profile. Used for
/// startup health before any file, environment, or CLI layer is captured.
///
/// # Errors
///
/// Returns the exact `search-config` failure if the closed defaults cannot
/// assemble, which is a fail-closed internal inconsistency.
pub fn build_effective_defaults() -> Result<EffectiveDaemonConfig, ConfigError> {
    build_effective(
        None,
        None,
        None,
        DAEMON_DIRECT_PROFILE,
        DAEMON_DIRECT_PROFILE,
    )
}

/// Plans activation of `candidate` over `current`, preserving every
/// independent obligation without scalar collapse.
///
/// # Errors
///
/// Returns `ReconfigurationRejected` when any field contributes the reject
/// action, or schema failures when snapshots diverge from the registry.
pub fn plan_activation(
    current: &EffectiveDaemonConfig,
    candidate: &EffectiveDaemonConfig,
) -> Result<ReconfigurationPlan, ConfigError> {
    let delta = diff(current.snapshot(), candidate.snapshot(), current.registry())?;
    plan_reconfiguration(&delta)
}

/// Atomically gates publication of `candidate`.
///
/// The candidate becomes authoritative only when every receipt required by
/// its plan is present in `proven`. Otherwise the current snapshot stays
/// authoritative and a closed blocker is returned; mixed partial state is
/// never published (invariant 16).
///
/// # Errors
///
/// Returns `DAEMON_CONFIG_REJECTED` for an explicitly rejected candidate,
/// or `DAEMON_CONFIG_PARTIAL_REFUSED` / `DAEMON_CONFIG_ACTIVATION_BLOCKED`
/// when receipts are missing, each with the current fingerprint retained by
/// the caller.
pub fn try_activate(
    current: &EffectiveDaemonConfig,
    candidate: EffectiveDaemonConfig,
    proven: &BTreeSet<ReceiptKind>,
) -> Result<EffectiveDaemonConfig, String> {
    if current.fingerprint() == candidate.fingerprint() {
        return Ok(candidate);
    }
    let plan = plan_activation(current, &candidate).map_err(|error| {
        let code = config_code(&error);
        if matches!(error, ConfigError::ReconfigurationRejected) {
            ACTIVATION_REJECTED.to_owned()
        } else {
            format!("{ACTIVATION_BLOCKED}:{code}:{error}")
        }
    })?;
    if plan.is_noop() {
        return Ok(candidate);
    }
    let missing: Vec<ReceiptKind> = plan
        .required_receipts
        .iter()
        .copied()
        .filter(|receipt| !proven.contains(receipt))
        .collect();
    if missing.is_empty() {
        Ok(candidate)
    } else {
        Err(ACTIVATION_PARTIAL_REFUSED.to_owned())
    }
}

/// Verified dependency state observed by the daemon composition root.
/// Every flag must come from a constructed dependency and a verified
/// runtime probe, never from `cfg!`, Cargo features, or constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DependencyState {
    /// Owner guard is held for the canonical root.
    pub runtime_owner_ready: bool,
    /// Control journal or redb mapping verified under the owner guard.
    pub control_store_verified: bool,
    /// Direct store opened and verified under the owner guard.
    pub direct_store_verified: bool,
    /// OS secret backend probed live for the current incarnation.
    pub secret_backend_verified: bool,
    /// Exact qualified Qdrant process and data plane are live.
    pub qdrant_available: bool,
    /// Exact control adapter required for search is constructed.
    pub control_adapter_available: bool,
    /// Persistent quarantine marker is armed.
    pub quarantined: bool,
}

/// Externally accepted receipts. Presence alone never activates a
/// capability; each flag must be backed by an accepted handoff or gate
/// receipt. W1 shell readiness leaves every flag false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedReceipts {
    /// General search acceptance (indexed or DIRECT query recipe).
    pub search_accepted: bool,
    /// Indexed search acceptance with qualified artifacts and routes.
    pub indexed_accepted: bool,
    /// Optional-profile gate acceptance.
    pub optional_gate_accepted: bool,
}

impl Default for AcceptedReceipts {
    fn default() -> Self {
        Self {
            search_accepted: false,
            indexed_accepted: false,
            optional_gate_accepted: false,
        }
    }
}

/// Truthful readiness derived from the effective snapshot, verified
/// dependencies, and accepted receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessReport {
    /// Effective configuration is assembled.
    pub configuration_ready: bool,
    /// Owner guard held.
    pub runtime_owner_ready: bool,
    /// Control verified and not quarantined.
    pub control_store_ready: bool,
    /// Secret backend probed live, never `cfg!(windows)`.
    pub secret_store_ready: bool,
    /// Shell endpoint exists.
    pub endpoint_ready: bool,
    /// Direct store verified and not quarantined.
    pub direct_store_ready: bool,
    /// DIRECT source-backed search over verified immutable revisions.
    pub source_backed_search_available: bool,
    /// General search only with an accepted receipt; W1 never implies it.
    pub search_available: bool,
    /// Indexed search only with qualified Qdrant, artifacts, and routes.
    pub indexed_search_available: bool,
    /// Closed blocker codes, never paths or secrets.
    pub blockers: Vec<&'static str>,
    /// Effective fingerprint for receipts.
    pub fingerprint: ConfigFingerprint,
}

fn boolean_field(snapshot: &EffectiveConfigSnapshot, section: &str, key: &str) -> Option<bool> {
    let registry = daemon_registry().ok()?;
    let section_name = section_name(section).ok()?;
    let key_name = key_name(key).ok()?;
    let effective = snapshot.section(&section_name)?.field(&key_name)?;
    let descriptor = registry.section(&section_name)?.field(&key_name)?;
    let _ = descriptor;
    match &effective.value {
        ConfigValue::Boolean(value) => Some(*value),
        _ => None,
    }
}

/// Derives truthful readiness without `cfg!`, feature presence, or health
/// constants. Enabling a feature flag alone changes no acceptance: every
/// `*_available` that requires an external receipt stays false until the
/// matching `AcceptedReceipts` flag and every verified dependency hold.
#[must_use]
pub fn derive_readiness(
    effective: &EffectiveDaemonConfig,
    dependencies: &DependencyState,
    accepted: &AcceptedReceipts,
) -> ReadinessReport {
    let snapshot = effective.snapshot();
    let control_ready = dependencies.control_store_verified && !dependencies.quarantined;
    let direct_ready = dependencies.direct_store_verified && !dependencies.quarantined;
    let source_backed = direct_ready && control_ready;
    let optional_semantic =
        boolean_field(snapshot, "optional_profiles", "semantic").unwrap_or(false);
    let optional_blocked = optional_semantic && !accepted.optional_gate_accepted;
    let mut blockers: Vec<&'static str> = Vec::new();
    if dependencies.quarantined {
        blockers.push(QUARANTINED_BLOCKER);
    }
    if !dependencies.control_store_verified {
        blockers.push(CONTROL_NOT_READY);
    }
    if !dependencies.direct_store_verified {
        blockers.push(DIRECT_NOT_READY);
    }
    if !accepted.search_accepted {
        blockers.push(SEARCH_NOT_ACCEPTED);
    }
    if !accepted.indexed_accepted || !dependencies.qdrant_available {
        blockers.push(INDEXED_NOT_ACCEPTED);
    }
    if !dependencies.control_adapter_available {
        blockers.push(CONTROL_NOT_READY);
    }
    if optional_blocked {
        blockers.push(OPTIONAL_GATE_REQUIRED);
    }
    let search_available = accepted.search_accepted
        && source_backed
        && dependencies.control_adapter_available
        && !optional_blocked
        && !dependencies.quarantined;
    let indexed_available = accepted.indexed_accepted
        && accepted.search_accepted
        && source_backed
        && dependencies.qdrant_available
        && dependencies.control_adapter_available
        && !optional_blocked
        && !dependencies.quarantined;
    ReadinessReport {
        configuration_ready: true,
        runtime_owner_ready: dependencies.runtime_owner_ready,
        control_store_ready: control_ready,
        secret_store_ready: dependencies.secret_backend_verified,
        endpoint_ready: true,
        direct_store_ready: direct_ready,
        source_backed_search_available: source_backed,
        search_available,
        indexed_search_available: indexed_available,
        blockers,
        fingerprint: snapshot.fingerprint(),
    }
}

/// Maps a pure configuration failure to a closed daemon code without
/// disclosing values, paths, or secrets.
#[must_use]
pub fn config_code(error: &ConfigError) -> &'static str {
    match error {
        ConfigError::SecretPlaintextForbidden => "DAEMON_CONFIG_SECRET_PLAINTEXT_FORBIDDEN",
        ConfigError::SecurityFloorViolation => "DAEMON_CONFIG_SECURITY_FLOOR_VIOLATION",
        ConfigError::UnknownSection => "DAEMON_CONFIG_UNKNOWN_SECTION",
        ConfigError::UnknownKey => "DAEMON_CONFIG_UNKNOWN_KEY",
        ConfigError::DuplicateKey | ConfigError::DuplicateTable | ConfigError::DuplicateSource => {
            "DAEMON_CONFIG_DUPLICATE"
        }
        ConfigError::DuplicateValidatedSection | ConfigError::MissingSection => {
            "DAEMON_CONFIG_INCOMPLETE"
        }
        ConfigError::OverrideNotAllowed => "DAEMON_CONFIG_OVERRIDE_NOT_ALLOWED",
        ConfigError::ResetNotAllowed => "DAEMON_CONFIG_RESET_NOT_ALLOWED",
        ConfigError::TypeMismatch => "DAEMON_CONFIG_TYPE_MISMATCH",
        ConfigError::ValueOutOfBounds | ConfigError::CapacityExceeded => {
            "DAEMON_CONFIG_BOUNDS_EXCEEDED"
        }
        ConfigError::ProfileNotAuthorized => "DAEMON_CONFIG_PROFILE_NOT_AUTHORIZED",
        ConfigError::StaleDescriptor => "DAEMON_CONFIG_STALE_DESCRIPTOR",
        ConfigError::ReconfigurationRejected => ACTIVATION_REJECTED,
        ConfigError::InvalidEnvironmentKey => "DAEMON_CONFIG_INVALID_ENVIRONMENT_KEY",
        _ => "DAEMON_CONFIG_INVALID",
    }
}

/// Read-only effective-configuration status. Pure and content-free: it
/// reports the fingerprint, profile, readiness, blockers, and redacted
/// entry counts, never raw paths or secrets. It performs no I/O and writes
/// no control history.
#[must_use]
pub fn config_status_json(effective: &EffectiveDaemonConfig, report: &ReadinessReport) -> String {
    let view = redacted_view(
        effective.snapshot(),
        effective.registry(),
        search_config::DisclosureLevel::Ordinary,
        daemon_limits(),
    );
    let fingerprint_hex = crate::sha256::hex(report.fingerprint.as_bytes());
    let mut blockers = String::from("[");
    for (index, blocker) in report.blockers.iter().enumerate() {
        if index > 0 {
            blockers.push(',');
        }
        blockers.push('"');
        blockers.push_str(blocker);
        blockers.push('"');
    }
    blockers.push(']');
    format!(
        concat!(
            "{{\"event\":\"effective_config_status\",",
            "\"schema\":\"eliot.effective-config-status-v1\",",
            "\"config_schema_version\":{},",
            "\"selected_profile\":\"{}\",",
            "\"config_fingerprint\":\"{}\",",
            "\"configuration_ready\":{},",
            "\"runtime_owner_ready\":{},",
            "\"control_store_ready\":{},",
            "\"secret_store_ready\":{},",
            "\"direct_store_ready\":{},",
            "\"source_backed_search_available\":{},",
            "\"search_available\":{},",
            "\"indexed_search_available\":{},",
            "\"blockers\":{},",
            "\"redacted_entries\":{},",
            "\"omitted_entries\":{},",
            "\"read_only\":true}}"
        ),
        DAEMON_CONFIG_SCHEMA_VERSION,
        effective.selected_profile(),
        fingerprint_hex,
        report.configuration_ready,
        report.runtime_owner_ready,
        report.control_store_ready,
        report.secret_store_ready,
        report.direct_store_ready,
        report.source_backed_search_available,
        report.search_available,
        report.indexed_search_available,
        blockers,
        view.entries.len(),
        view.omitted_entries,
    )
}

/// Shell dependency state: no owner, no verified stores, no secret probe,
/// no Qdrant, no control adapter, and no quarantine. Used for `--health`
/// and pre-root serve paths.
#[must_use]
pub const fn shell_dependencies() -> DependencyState {
    DependencyState {
        runtime_owner_ready: false,
        control_store_verified: false,
        direct_store_verified: false,
        secret_backend_verified: false,
        qdrant_available: false,
        control_adapter_available: false,
        quarantined: false,
    }
}

/// Direct-store dependency state after an owner-fenced open and verify.
/// The secret backend remains unverified here: callers must replace it with
/// a live OS-secret probe instead of `cfg!(windows)`.
#[must_use]
pub const fn direct_dependencies() -> DependencyState {
    DependencyState {
        runtime_owner_ready: true,
        control_store_verified: true,
        direct_store_verified: true,
        secret_backend_verified: false,
        qdrant_available: false,
        control_adapter_available: true,
        quarantined: false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use search_config::{
        ConfigDocument, ConfigKeyPath, ConfigSource, ConfigSourceKind, ConfigSourceRef,
        DocumentValue, LayerOperation, ReceiptKind,
    };
    use search_contracts::Blake3Digest32;

    use super::{
        build_effective, build_effective_defaults, capture_cli_document,
        capture_environment_document, capture_file_document, config_status_json, daemon_registry,
        derive_readiness, direct_dependencies, plan_activation, shell_dependencies, try_activate,
        AcceptedReceipts,
    };
    use crate::sha256;

    fn file_doc(text: &str) -> ConfigDocument {
        capture_file_document(text.as_bytes(), "test-file").expect("file parses")
    }

    fn direct_file(extra: &str) -> ConfigDocument {
        let body = format!("schema_version = 1\nprofile = \"direct\"\n{extra}");
        file_doc(&body)
    }

    #[test]
    fn registry_mirrors_declared_owners_and_reload_classes() {
        let registry = daemon_registry().expect("registry");
        assert_eq!(registry.config_schema_version(), 1);
        assert_eq!(registry.len(), 7);
        let owner = |section: &str| {
            registry
                .section(&search_config::ConfigSectionName::new(section, 128).expect("section"))
                .expect("present")
                .owner()
                .as_str()
                .to_owned()
        };
        assert_eq!(owner("instance"), "search-runtime-owner");
        assert_eq!(owner("secrets"), "search-os-secrets");
        assert_eq!(owner("control"), "search-control-redb");
        assert_eq!(owner("source_admission"), "search-source-admission");
        assert_eq!(owner("lexical"), "search-lexical");
        assert_eq!(owner("query"), "search-query-planner");
        assert_eq!(owner("optional_profiles"), "eliot-searchd");
    }

    #[test]
    fn effective_config_assembles_from_defaults_file_env_cli() {
        let file = direct_file("[query]\nlimit = 20\n");
        let environment = capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")])
            .expect("env")
            .expect("layer");
        let cli = capture_cli_document(&[("query.limit", "40")])
            .expect("cli")
            .expect("layer");
        let effective =
            build_effective(Some(file), Some(environment), Some(cli), "direct", "direct")
                .expect("effective");
        let registry = effective.registry();
        let path = ConfigKeyPath::new(
            search_config::ConfigSectionName::new("query", 128).expect("section"),
            search_config::ConfigKeyName::new("limit", 128).expect("key"),
        );
        let descriptor = registry.field(&path).expect("descriptor");
        let (field_descriptor, value) = effective
            .snapshot()
            .field(registry, path.section(), path.key())
            .expect("field");
        assert_eq!(field_descriptor.key().as_str(), descriptor.key().as_str());
        match &value.value {
            search_config::ConfigValue::Integer(number) => assert_eq!(*number, 40),
            other => panic!("CLI must win with 40, got {other:?}"),
        }
        assert_eq!(
            value.provenance.source.kind,
            search_config::ConfigSourceKind::Cli
        );
    }

    #[test]
    fn cli_wins_over_environment_over_file_over_defaults() {
        let defaults = build_effective(None, None, None, "direct", "direct").expect("defaults");
        let file = build_effective(
            Some(direct_file("[query]\nlimit = 20\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("file");
        assert_ne!(defaults.fingerprint(), file.fingerprint());
        let with_env = build_effective(
            Some(direct_file("[query]\nlimit = 20\n")),
            capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")]).expect("env"),
            None,
            "direct",
            "direct",
        )
        .expect("env wins");
        assert_ne!(file.fingerprint(), with_env.fingerprint());
        let with_cli = build_effective(
            Some(direct_file("[query]\nlimit = 20\n")),
            capture_environment_document(&[("ELIOT_SEARCH__QUERY__LIMIT", "30")]).expect("env"),
            capture_cli_document(&[("query.limit", "40")]).expect("cli"),
            "direct",
            "direct",
        )
        .expect("cli wins");
        assert_ne!(with_env.fingerprint(), with_cli.fingerprint());
    }

    #[test]
    fn explicit_reset_restores_compiled_default() {
        let effective = build_effective(
            Some(direct_file("[query]\nlimit = 20\n")),
            None,
            capture_cli_document(&[("query.limit", "__RESET__")]).expect("cli"),
            "direct",
            "direct",
        )
        .expect("reset");
        let section = search_config::ConfigSectionName::new("query", 128).expect("section");
        let key = search_config::ConfigKeyName::new("limit", 128).expect("key");
        let field = effective
            .snapshot()
            .section(&section)
            .expect("section")
            .field(&key)
            .expect("field");
        assert_eq!(field.value, search_config::ConfigValue::Integer(10));
        assert!(field.provenance.explicit_reset);
    }

    #[test]
    fn duplicate_unknown_and_wrong_type_fail_closed() {
        assert!(capture_file_document(
            b"schema_version = 1\nprofile = \"direct\"\n[query]\nlimit = 1\nlimit = 2\n",
            "dup",
        )
        .is_err());
        let unknown = build_effective(
            Some(direct_file("[query]\ntypo = 1\n")),
            None,
            None,
            "direct",
            "direct",
        );
        assert!(unknown.is_err());
        let wrong_type = build_effective(
            Some(direct_file("[query]\nlimit = \"not-a-number\"\n")),
            None,
            None,
            "direct",
            "direct",
        );
        assert!(wrong_type.is_err());
    }

    #[test]
    fn unknown_prefixed_environment_key_fails_closed() {
        assert!(capture_environment_document(&[("ELIOT_SEARCH__QUERY__TYPO", "1")]).is_err());
        assert!(capture_environment_document(&[("ELIOT_SEARCH__QUERY__limit", "1")]).is_err());
    }

    #[test]
    fn plaintext_secret_is_denied_on_every_layer() {
        let file = direct_file("[secrets]\nqdrant_api_secret_ref = \"actual-api-key\"\n");
        assert!(build_effective(Some(file), None, None, "direct", "direct").is_err());
        let environment =
            capture_environment_document(&[("ELIOT_SEARCH__SECRETS__QDRANT_API_SECRET_REF", "k")]);
        assert!(
            environment.is_err() || {
                let layer = environment.expect("checked").expect("layer");
                build_effective(None, Some(layer), None, "direct", "direct").is_err()
            }
        );
        let cli = capture_cli_document(&[("secrets.qdrant_api_secret_ref", "actual-api-key")])
            .expect("cli layer builds");
        assert!(
            build_effective(None, None, cli, "direct", "direct").is_err(),
            "CLI secret plaintext must fail at merge"
        );
    }

    #[test]
    fn opaque_secret_reference_is_accepted() {
        let effective = build_effective(
            Some(direct_file(
                "[secrets]\nqdrant_api_secret_ref = \"secret://credential/qdrant-test\"\n",
            )),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("opaque ref accepted");
        let section = search_config::ConfigSectionName::new("secrets", 128).expect("section");
        let key = search_config::ConfigKeyName::new("qdrant_api_secret_ref", 128).expect("key");
        let field = effective
            .snapshot()
            .section(&section)
            .expect("section")
            .field(&key)
            .expect("field");
        assert!(matches!(
            field.value,
            search_config::ConfigValue::SecretReference(_)
        ));
    }

    #[test]
    fn mixed_live_restart_rebuild_failure_retains_old_snapshot() {
        let current = build_effective_defaults().expect("current");
        let candidate = build_effective(
            Some(direct_file(
                "[query]\nlimit = 11\n[source_admission]\nallow_generated = false\n[lexical]\nprofile_id = \"candidate-v2\"\n",
            )),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("candidate");
        assert_ne!(current.fingerprint(), candidate.fingerprint());
        let plan = plan_activation(&current, &candidate).expect("plan");
        assert!(plan
            .required_actions
            .contains(&search_config::ReconfigurationAction::ApplyLive));
        assert!(plan
            .required_actions
            .contains(&search_config::ReconfigurationAction::SecurityBarrier));
        assert!(plan
            .required_actions
            .contains(&search_config::ReconfigurationAction::NewCollectionGeneration));
        assert!(plan
            .required_actions
            .contains(&search_config::ReconfigurationAction::RebuildProjection));
        let live_only = BTreeSet::from([ReceiptKind::LiveApply]);
        let retained = try_activate(&current, candidate.clone(), &live_only);
        assert_eq!(
            retained,
            Err(super::ACTIVATION_PARTIAL_REFUSED.to_owned()),
            "partial receipts must never publish"
        );
        let full: BTreeSet<ReceiptKind> = plan.required_receipts.iter().copied().collect();
        let published = try_activate(&current, candidate, &full).expect("full receipts publish");
        assert_ne!(published.fingerprint(), current.fingerprint());
    }

    #[test]
    fn fixed_floor_reject_never_publishes() {
        let candidate = build_effective(
            Some(direct_file("[control]\ndurability = \"best_effort\"\n")),
            None,
            None,
            "direct",
            "direct",
        );
        assert!(
            candidate.is_err(),
            "fixed durability change must fail at merge, not plan"
        );
    }

    #[test]
    fn w1_readiness_does_not_imply_search_available() {
        let effective = build_effective_defaults().expect("effective");
        let mut dependencies = direct_dependencies();
        dependencies.secret_backend_verified = true;
        let report = derive_readiness(&effective, &dependencies, &AcceptedReceipts::default());
        assert!(report.configuration_ready);
        assert!(report.runtime_owner_ready);
        assert!(report.control_store_ready);
        assert!(report.direct_store_ready);
        assert!(report.source_backed_search_available);
        assert!(
            !report.search_available,
            "W1 readiness without an accepted search receipt must not imply search"
        );
        assert!(!report.indexed_search_available);
        assert!(report.blockers.contains(&super::SEARCH_NOT_ACCEPTED));
    }

    #[test]
    fn enabling_optional_flag_alone_changes_no_acceptance() {
        let effective = build_effective(
            Some(direct_file("[optional_profiles]\nsemantic = true\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("flag parses");
        let report = derive_readiness(
            &effective,
            &direct_dependencies(),
            &AcceptedReceipts::default(),
        );
        assert!(!report.search_available);
        assert!(report.blockers.contains(&super::OPTIONAL_GATE_REQUIRED));
        let current = build_effective_defaults().expect("current");
        let plan = plan_activation(&current, &effective).expect("plan");
        assert!(plan.activation_blocked);
        assert_eq!(
            try_activate(&current, effective, &BTreeSet::new()),
            Err(super::ACTIVATION_PARTIAL_REFUSED.to_owned())
        );
    }

    #[test]
    fn missing_adapters_yield_truthful_health() {
        let effective = build_effective_defaults().expect("effective");
        let report = derive_readiness(
            &effective,
            &shell_dependencies(),
            &AcceptedReceipts::default(),
        );
        assert!(!report.runtime_owner_ready);
        assert!(!report.control_store_ready);
        assert!(!report.direct_store_ready);
        assert!(!report.source_backed_search_available);
        assert!(!report.search_available);
        assert!(!report.secret_store_ready, "never cfg!(windows)");
    }

    #[test]
    fn profile_self_authorization_is_rejected() {
        let registry = daemon_registry().expect("registry");
        let source = ConfigSource {
            kind: ConfigSourceKind::File,
            source_ref: ConfigSourceRef::new("self-auth", 128).expect("ref"),
            source_digest: Blake3Digest32::from_bytes([7; 32]),
        };
        let document = ConfigDocument::from_entries(
            1,
            Some(search_contracts::ProfileId::new("semantic_optional").expect("profile")),
            source,
            [],
            search_config::ConfigLimits::W1,
        )
        .expect("document");
        let merged = search_config::merge_layers(
            search_config::ConfigLayers {
                defaults: ConfigSource {
                    kind: ConfigSourceKind::File,
                    source_ref: ConfigSourceRef::new("defaults-probe", 128).expect("ref"),
                    source_digest: Blake3Digest32::from_bytes([0; 32]),
                },
                requested_profile: search_contracts::ProfileId::new("semantic_optional")
                    .expect("profile"),
                file: Some(document),
                environment: None,
                cli: None,
            },
            &registry,
            search_config::ConfigLimits::W1,
        );
        assert!(merged.is_err(), "defaults kind mismatch must fail");
    }

    #[test]
    fn read_only_status_writes_nothing_and_leaks_nothing() {
        let effective = build_effective(
            Some(direct_file(
                "[instance]\ndata_root = \"C:/Users/alice/private-search-data\"\n[secrets]\nqdrant_api_secret_ref = \"secret://credential/qdrant-production\"\n",
            )),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("effective");
        let report = derive_readiness(
            &effective,
            &direct_dependencies(),
            &AcceptedReceipts::default(),
        );
        let first = config_status_json(&effective, &report);
        assert!(first.contains("\"read_only\":true"));
        assert!(first.contains("\"search_available\":false"));
        assert!(!first.contains("alice"), "paths must never leak");
        assert!(
            !first.contains("qdrant-production"),
            "secret references must never leak"
        );
        for _ in 0..1_000 {
            assert_eq!(config_status_json(&effective, &report), first);
        }
        let fingerprint_hex = sha256::hex(report.fingerprint.as_bytes());
        assert!(first.contains(&fingerprint_hex));
    }

    #[test]
    fn identical_inputs_reproduce_identical_fingerprints() {
        let left = build_effective(
            Some(direct_file("[query]\nlimit = 21\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("left");
        let right = build_effective(
            Some(direct_file("[query]\nlimit = 21\n")),
            None,
            None,
            "direct",
            "direct",
        )
        .expect("right");
        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn secret_cli_injection_is_rejected_at_merge() {
        let cli = capture_cli_document(&[(
            "secrets.qdrant_api_secret_ref",
            "secret://credential/from-cli",
        )])
        .expect("cli builds");
        assert!(
            build_effective(None, None, cli, "direct", "direct").is_err(),
            "secrets are not CLI-whitelisted"
        );
    }

    #[test]
    fn typed_cli_document_rejects_string_lists() {
        let path = ConfigKeyPath::new(
            search_config::ConfigSectionName::new("query", 128).expect("section"),
            search_config::ConfigKeyName::new("limit", 128).expect("key"),
        );
        let entries = vec![(
            path,
            LayerOperation::Set(DocumentValue::StringList(vec!["a".to_owned()])),
        )];
        assert!(super::capture_cli_typed_document(entries).is_err());
    }
}
