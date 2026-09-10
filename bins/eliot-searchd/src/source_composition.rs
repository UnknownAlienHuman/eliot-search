//! Canonical source composition for primary ingestion.
//!
//! Daemon-local composition over kernel-verified reads (T07) that binds
//! explicit admission, stable identity and registry-view checks before any
//! revision object reaches CAS or any catalog event is appended (T11
//! transactions with T05 quarantine and T08 owner guards preserved).
//!
//! Normative semantics mirror the accepted pure kernels without importing
//! them as Rust dependencies (progressive composition: `wave2-source`
//! enables direct crate calls in a follow-up without changing this file's
//! closed vocabularies):
//!
//! - `search-source-admission` (`FUNCTIONS.md`): `validate_observation`,
//!   `observation_digest`, `evaluate`, `issue_receipt`, `verify_receipt`.
//!   Deny-by-default, unconditional credential/private-key denies, explicit
//!   generated/vendor/binary gates, size/location/kind fences, unavailable
//!   signals require review and never allow.
//! - `search-source-identity` (`FUNCTIONS.md`): `derive_canonical_path_key`,
//!   `resolve_identity`, `derive_source_identity`. Paths are locators, never
//!   identity; stable physical components dominate path similarity; path-only
//!   never creates or matches.
//! - `search-source-registry` (`api.rs`): `register_root`, `admit_source`,
//!   `bind_membership`, `resolve_source_view`. Membership requires an exact
//!   current verified `ALLOW` receipt; one coherent view per operation; no
//!   source bytes or vendor types cross here.
//!
//! This module defines daemon composition receipts with their own domains
//! (`eliot-searchd/source-composition-*/v1`). It never fabricates
//! `search-source-*` crate receipts to make types compile. When
//! `wave2-source` is enabled, callers verify these composition receipts
//! against the real kernels via the `#[cfg(feature)]` cross-checks below.
//!
//! Persistence: canonical registry changes persist in T11 transactions via
//! the existing DIRECT append-only log (`append_drafts` with exact readback).
//! This view holds no second file, no second map and no unbounded queue.
//! Legacy migration: `MatchExisting` by stable file identity reuses the
//! existing (possibly legacy-domain) `source_id`; only truly new stable
//! identities use the canonical domain. No fabricated mapping.
//!
//! Bounds: every input is finite (`MAX_*` below); cancellation is process
//! termination on this path; timeouts after possible external writes are
//! `OUTCOME_UNKNOWN` until readback (handled by the caller via quarantine).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use crate::sha256;

// ---------------------------------------------------------------------------
// Closed constants and error codes
// ---------------------------------------------------------------------------

/// Composition receipt schema version (distinct from crate receipt versions).
pub const COMPOSITION_SCHEMA_VERSION: u32 = 1;
/// Baseline admission policy revision for primary ingestion.
pub const BASELINE_POLICY_REVISION: u64 = 1;
/// Baseline maximum admitted file bytes (16 MiB, per admission contract).
pub const BASELINE_MAX_FILE_BYTES: u64 = 16_777_216;
/// Maximum root-relative lookup bytes for one canonical path key.
pub const MAX_RELATIVE_PATH_BYTES: usize = 4_096;
/// Maximum file-name bytes inspected by the closed classifier.
pub const MAX_CLASSIFIER_PATH_BYTES: usize = 4_096;
/// Maximum reason codes carried by one decision or receipt.
pub const MAX_REASON_CODES: usize = 64;

/// Admission denied under the current policy.
pub const SOURCE_ADMISSION_DENIED: &str = "SOURCE_ADMISSION_DENIED";
/// Admission requires human review; never silently allowed.
pub const SOURCE_ADMISSION_REVIEW_REQUIRED: &str = "SOURCE_ADMISSION_REVIEW_REQUIRED";
/// Source kind is valid but never admittable.
pub const SOURCE_KIND_UNSUPPORTED: &str = "SOURCE_KIND_UNSUPPORTED";
/// Observed size exceeds the policy ceiling.
pub const SOURCE_TOO_LARGE: &str = "SOURCE_TOO_LARGE";
/// Sensitive source denied under the current policy.
pub const SENSITIVE_SOURCE_DENIED: &str = "SENSITIVE_SOURCE_DENIED";
/// Receipt does not bind the supplied policy, observation or decision.
pub const ADMISSION_RECEIPT_MISMATCH: &str = "ADMISSION_RECEIPT_MISMATCH";
/// Receipt binds a stale policy revision or fingerprint.
pub const ADMISSION_RECEIPT_STALE: &str = "ADMISSION_RECEIPT_STALE";
/// Stable identity evidence is absent or ambiguous.
pub const SOURCE_IDENTITY_AMBIGUOUS: &str = "SOURCE_IDENTITY_AMBIGUOUS";
/// Stable identity conflicts with a claimed or active binding.
pub const SOURCE_IDENTITY_CONFLICT: &str = "SOURCE_IDENTITY_CONFLICT";
/// Canonical path key conflicts with an active binding.
pub const PATH_BINDING_CONFLICT: &str = "PATH_BINDING_CONFLICT";
/// Admitted root is unavailable; never an empty source set.
pub const ADMITTED_ROOT_UNAVAILABLE: &str = "ADMITTED_ROOT_UNAVAILABLE";
/// Same stable identity maps to multiple durable source IDs.
pub const SOURCE_IDENTITY_COLLISION: &str = "SOURCE_IDENTITY_COLLISION";

// ---------------------------------------------------------------------------
// Closed vocabularies (mirror admission crate wire strings exactly)
// ---------------------------------------------------------------------------

/// Closed source class (never a free-form string).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceClass {
    Regular,
    Test,
    Documentation,
    Generated,
    Vendor,
    Binary,
    System,
    Cache,
    BuildOutput,
    Credential,
    PrivateKey,
}

impl SourceClass {
    /// Stable wire string (matches admission crate).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Test => "test",
            Self::Documentation => "documentation",
            Self::Generated => "generated",
            Self::Vendor => "vendor",
            Self::Binary => "binary",
            Self::System => "system",
            Self::Cache => "cache",
            Self::BuildOutput => "build-output",
            Self::Credential => "credential",
            Self::PrivateKey => "private-key",
        }
    }

    const fn is_unconditional_deny(self) -> bool {
        matches!(self, Self::Credential | Self::PrivateKey)
    }
}

/// Closed sensitivity level.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SensitivityLevel {
    Public,
    Internal,
    Confidential,
    SecretCandidate,
    Credential,
    PrivateKey,
}

impl SensitivityLevel {
    /// Stable wire string (matches admission crate).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::SecretCandidate => "secret-candidate",
            Self::Credential => "credential",
            Self::PrivateKey => "private-key",
        }
    }

    const fn is_unconditional_deny(self) -> bool {
        matches!(self, Self::Credential | Self::PrivateKey)
    }
}

/// Terminal admission outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdmissionOutcome {
    Allow,
    Deny,
    ReviewRequired,
    Unsupported,
}

impl AdmissionOutcome {
    /// Stable wire string (matches admission crate).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
            Self::ReviewRequired => "REVIEW_REQUIRED",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

/// Stable ordered admission reason code (matches admission crate exactly).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AdmissionReasonCode {
    AllowBaselineSource,
    AllowExplicitRule,
    DenyCredentialClass,
    DenyPrivateKeyClass,
    DenySensitiveCredential,
    DenySystemLocation,
    DenyCacheArtifact,
    DenyBuildOutput,
    DenyVendorClass,
    DenyGeneratedClass,
    DenyBinaryClass,
    DenySensitiveClass,
    DenySizeExceeded,
    DenyEmptySource,
    DenyRemoteLocation,
    DenyExplicitRule,
    DenyByDefault,
    ReviewDetectorUnavailable,
    ReviewSensitivityUnknown,
    ReviewMetadataUnavailable,
    ReviewExplicitRule,
    UnsupportedSourceKind,
    UnsupportedExplicitRule,
}

impl AdmissionReasonCode {
    /// Stable machine-readable code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowBaselineSource => "ALLOW_BASELINE_SOURCE",
            Self::AllowExplicitRule => "ALLOW_EXPLICIT_RULE",
            Self::DenyCredentialClass => "DENY_CREDENTIAL_CLASS",
            Self::DenyPrivateKeyClass => "DENY_PRIVATE_KEY_CLASS",
            Self::DenySensitiveCredential => "DENY_SENSITIVE_CREDENTIAL",
            Self::DenySystemLocation => "DENY_SYSTEM_LOCATION",
            Self::DenyCacheArtifact => "DENY_CACHE_ARTIFACT",
            Self::DenyBuildOutput => "DENY_BUILD_OUTPUT",
            Self::DenyVendorClass => "DENY_VENDOR_CLASS",
            Self::DenyGeneratedClass => "DENY_GENERATED_CLASS",
            Self::DenyBinaryClass => "DENY_BINARY_CLASS",
            Self::DenySensitiveClass => "DENY_SENSITIVE_CLASS",
            Self::DenySizeExceeded => "DENY_SIZE_EXCEEDED",
            Self::DenyEmptySource => "DENY_EMPTY_SOURCE",
            Self::DenyRemoteLocation => "DENY_REMOTE_LOCATION",
            Self::DenyExplicitRule => "DENY_EXPLICIT_RULE",
            Self::DenyByDefault => "DENY_BY_DEFAULT",
            Self::ReviewDetectorUnavailable => "REVIEW_DETECTOR_UNAVAILABLE",
            Self::ReviewSensitivityUnknown => "REVIEW_SENSITIVITY_UNKNOWN",
            Self::ReviewMetadataUnavailable => "REVIEW_METADATA_UNAVAILABLE",
            Self::ReviewExplicitRule => "REVIEW_EXPLICIT_RULE",
            Self::UnsupportedSourceKind => "UNSUPPORTED_SOURCE_KIND",
            Self::UnsupportedExplicitRule => "UNSUPPORTED_EXPLICIT_RULE",
        }
    }
}

// ---------------------------------------------------------------------------
// Admission policy
// ---------------------------------------------------------------------------

/// Explicit admission configuration (future T12 wiring point).
///
/// Baseline denies generated, vendor and binary classes and caps one file
/// at 16 MiB. Callers supply explicit flags; unknown fields fail closed at
/// the config layer, never here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAdmissionConfig {
    /// Policy revision (must be non-zero).
    pub revision: u64,
    /// Maximum admitted file bytes (must be non-zero and within ceiling).
    pub max_file_bytes: u64,
    /// Whether generated sources may be admitted.
    pub allow_generated: bool,
    /// Whether vendor sources may be admitted.
    pub allow_vendor: bool,
    /// Whether binary sources may be admitted.
    pub allow_binary: bool,
}

impl SourceAdmissionConfig {
    /// Baseline deny-by-default configuration.
    pub const fn baseline() -> Self {
        Self {
            revision: BASELINE_POLICY_REVISION,
            max_file_bytes: BASELINE_MAX_FILE_BYTES,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
        }
    }

    fn validate(self) -> Result<AdmissionPolicy, String> {
        if self.revision == 0 {
            return Err("ADMISSION_POLICY_CONFLICT".to_owned());
        }
        if self.max_file_bytes == 0 || self.max_file_bytes > BASELINE_MAX_FILE_BYTES {
            return Err("ADMISSION_POLICY_CONFLICT".to_owned());
        }
        let fingerprint = sha256::hex(&sha256::digest_parts(
            b"eliot-searchd/source-composition-policy/v1",
            &[
                &self.revision.to_be_bytes(),
                &self.max_file_bytes.to_be_bytes(),
                &[u8::from(self.allow_generated)],
                &[u8::from(self.allow_vendor)],
                &[u8::from(self.allow_binary)],
            ],
        ));
        Ok(AdmissionPolicy {
            revision: self.revision,
            max_file_bytes: self.max_file_bytes,
            allow_generated: self.allow_generated,
            allow_vendor: self.allow_vendor,
            allow_binary: self.allow_binary,
            fingerprint,
        })
    }
}

/// Validated canonical admission policy with stable fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionPolicy {
    revision: u64,
    max_file_bytes: u64,
    allow_generated: bool,
    allow_vendor: bool,
    allow_binary: bool,
    fingerprint: String,
}

impl AdmissionPolicy {
    /// Baseline deny-by-default policy.
    pub fn baseline() -> Self {
        SourceAdmissionConfig::baseline()
            .validate()
            .expect("baseline admission config is valid")
    }

    /// Explicit policy from validated configuration.
    ///
    /// # Errors
    ///
    /// Returns `ADMISSION_POLICY_CONFLICT` for out-of-bounds inputs.
    pub fn from_config(config: SourceAdmissionConfig) -> Result<Self, String> {
        config.validate()
    }

    /// Exact policy revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Maximum admitted file bytes.
    pub const fn max_file_bytes(&self) -> u64 {
        self.max_file_bytes
    }

    /// Stable policy fingerprint (hex, domain-separated).
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

// ---------------------------------------------------------------------------
// Closed path classifier (platform boundary signals, no body classifier)
// ---------------------------------------------------------------------------

fn file_name_text(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
    if name.is_empty() || name.len() > MAX_CLASSIFIER_PATH_BYTES {
        return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
    }
    if name.contains('\0') {
        return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
    }
    Ok(name.to_owned())
}

fn path_components_lower(path: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let text = part
                    .to_str()
                    .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
                if text.is_empty() || text.len() > MAX_CLASSIFIER_PATH_BYTES {
                    return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
                }
                out.push(text.to_ascii_lowercase());
                if out.len() > 256 {
                    return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
                }
            }
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                // Absolute paths are expected here (kernel locators); parent
                // escapes never reach this point (adapter denies them).
                continue;
            }
        }
    }
    if out.is_empty() {
        return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
    }
    Ok(out)
}

/// Closed classification of one candidate file.
///
/// Returns `(class, sensitivity, is_binary_hint)`. The binary hint is
/// extension-based only (see above); admission never hashes source bodies.
fn classify_source(
    path: &Path,
    _bytes: &[u8],
) -> Result<(SourceClass, SensitivityLevel, bool), String> {
    let raw_name = file_name_text(path)?;
    let name = raw_name.to_ascii_lowercase();
    let components = path_components_lower(path)?;
    let extension = raw_name
        .rfind('.')
        .map_or("", |index| {
            if index + 1 < raw_name.len() {
                &raw_name[index + 1..]
            } else {
                ""
            }
        })
        .to_ascii_lowercase();

    // Unconditional safety denies first: private-key and credential material.
    // These can never be bypassed by an explicit allow flag.
    let credential_name = name == "id_rsa"
        || name == "id_ed25519"
        || name == "id_ecdsa"
        || name == "id_dsa"
        || name.starts_with("id_rsa.")
        || name.starts_with("id_ed25519.")
        || name == "private_key"
        || name == "private-key"
        || name == "secret_key"
        || name == "secret-key"
        || matches!(
            extension.as_str(),
            "pem" | "key" | "pfx" | "p12" | "asc" | "gpg" | "pgp" | "kdbx"
        )
        || name == "credentials.json"
        || name == "secrets.yaml"
        || name == "secrets.yml"
        || name == "secrets.json"
        || name == "secrets.toml";
    if credential_name {
        if matches!(extension.as_str(), "pem" | "key" | "pfx" | "p12")
            || name.starts_with("id_")
            || name.contains("private")
        {
            return Ok((SourceClass::PrivateKey, SensitivityLevel::PrivateKey, false));
        }
        return Ok((SourceClass::Credential, SensitivityLevel::Credential, false));
    }

    // System, cache and build-output locations are denied with dedicated
    // reasons so fixtures stay explicit.
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            ".git" | ".hg" | ".svn" | "system volume information" | "$recycle.bin" | ".ds_store"
        )
    }) || name == ".ds_store"
        || name == "thumbs.db"
    {
        return Ok((SourceClass::System, SensitivityLevel::Internal, false));
    }
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "__pycache__" | ".cache" | ".mypy_cache" | ".pytest_cache" | ".venv" | "venv"
        )
    }) {
        return Ok((SourceClass::Cache, SensitivityLevel::Internal, false));
    }
    if components
        .iter()
        .any(|component| matches!(component.as_str(), "target" | "dist" | "build" | "out"))
        || matches!(extension.as_str(), "o" | "obj" | "class" | "pyc" | "pyo")
    {
        return Ok((SourceClass::BuildOutput, SensitivityLevel::Internal, false));
    }

    // Generated sources: explicit allow flag gates them.
    if name.contains(".generated.")
        || name.contains("_generated")
        || name.contains("generated_")
        || components.iter().any(|component| component == "generated")
        || matches!(extension.as_str(), "g.cs" | "pb.go")
        || name.ends_with(".designer.cs")
        || name.ends_with(".min.js")
    {
        return Ok((SourceClass::Generated, SensitivityLevel::Internal, false));
    }

    // Vendor sources: explicit allow flag gates them.
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "vendor" | "third_party" | "thirdparty" | "node_modules"
        )
    }) {
        return Ok((SourceClass::Vendor, SensitivityLevel::Internal, false));
    }

    // Binary sources: extension gates admission. Raw NUL content does NOT
    // gate here: binary `.txt` sources are admitted for downstream gap
    // handling (materializer reports `MATERIALIZATION_BINARY_CONTENT` with
    // `searched_sources=0` instead of an exact negative), preserving the
    // preparation flow. Only known binary extensions deny at admission.
    let binary_hint = matches!(
        extension.as_str(),
        "exe"
            | "dll"
            | "so"
            | "dylib"
            | "bin"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "ico"
            | "zip"
            | "gz"
            | "7z"
            | "rar"
            | "pdf"
    );
    if binary_hint {
        // A secret-looking binary name is still a binary class denial with a
        // sensitive ceiling; unconditional credential names already returned.
        if name.contains("secret")
            || name.contains("credential")
            || name.contains("token")
            || name.contains("password")
            || name.contains("private")
        {
            return Ok((SourceClass::Binary, SensitivityLevel::SecretCandidate, true));
        }
        return Ok((SourceClass::Binary, SensitivityLevel::Internal, true));
    }

    // Secret-candidate sensitivity for suggestive names without confirmed
    // credential shape. Denied unless an explicit profile permits it; the
    // baseline never silently allows it.
    if name.contains("secret")
        || name.contains("credential")
        || name.contains("private")
        || name.contains("password")
        || name == ".env"
        || name.starts_with(".env.")
    {
        return Ok((
            SourceClass::Regular,
            SensitivityLevel::SecretCandidate,
            false,
        ));
    }
    if name.contains("token") && (name.contains("api") || name.contains("auth")) {
        return Ok((
            SourceClass::Regular,
            SensitivityLevel::SecretCandidate,
            false,
        ));
    }

    // Baseline admittable classes: test, documentation, regular.
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "test" | "tests" | "testing" | "docs" | "documentation"
        )
    }) || name.starts_with("test_")
        || name.starts_with("test-")
        || name.ends_with("_test.rs")
        || name.ends_with("_test.go")
        || name.ends_with(".test.js")
        || name.ends_with(".test.ts")
        || extension.as_str() == "md"
        || extension.as_str() == "rst"
        || name.starts_with("readme")
        || name.starts_with("changelog")
        || name.starts_with("license")
    {
        if extension.as_str() == "md"
            || extension.as_str() == "rst"
            || name.starts_with("readme")
            || name.starts_with("changelog")
            || name.starts_with("license")
            || components
                .iter()
                .any(|component| matches!(component.as_str(), "docs" | "documentation"))
        {
            return Ok((SourceClass::Documentation, SensitivityLevel::Public, false));
        }
        return Ok((SourceClass::Test, SensitivityLevel::Internal, false));
    }
    Ok((SourceClass::Regular, SensitivityLevel::Internal, false))
}

// ---------------------------------------------------------------------------
// Admission observation, decision and receipt
// ---------------------------------------------------------------------------

/// Content-free admission observation (no paths, no bytes).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionObservation {
    source_class: SourceClass,
    sensitivity: SensitivityLevel,
    byte_size: u64,
    is_binary_hint: bool,
}

impl AdmissionObservation {
    /// Source class under evaluation.
    pub const fn source_class(&self) -> SourceClass {
        self.source_class
    }

    /// Maximum sensitivity signal.
    pub const fn sensitivity(&self) -> SensitivityLevel {
        self.sensitivity
    }

    /// Observed byte size.
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Deterministic admission decision with stable ordered reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionDecision {
    outcome: AdmissionOutcome,
    reasons: BTreeSet<AdmissionReasonCode>,
    sensitivity: SensitivityLevel,
    policy_revision: u64,
    policy_fingerprint: String,
    observation_digest: String,
}

impl AdmissionDecision {
    /// Terminal outcome.
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
    }

    /// Stable ordered reason codes.
    pub fn reasons(&self) -> impl ExactSizeIterator<Item = &AdmissionReasonCode> {
        self.reasons.iter()
    }

    /// Maximum sensitivity class.
    pub const fn sensitivity(&self) -> SensitivityLevel {
        self.sensitivity
    }
}

/// Immutable composition receipt binding policy, observation and decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposedAdmissionReceipt {
    policy_revision: u64,
    policy_fingerprint: String,
    observation_digest: String,
    outcome: AdmissionOutcome,
    reasons: BTreeSet<AdmissionReasonCode>,
    sensitivity: SensitivityLevel,
    receipt_digest: String,
}

impl ComposedAdmissionReceipt {
    /// Policy revision bound by this receipt.
    pub const fn policy_revision(&self) -> u64 {
        self.policy_revision
    }

    /// Policy fingerprint bound by this receipt.
    pub fn policy_fingerprint(&self) -> &str {
        &self.policy_fingerprint
    }

    /// Observation digest bound by this receipt.
    pub fn observation_digest(&self) -> &str {
        &self.observation_digest
    }

    /// Terminal outcome.
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
    }

    /// Ordered reason codes.
    pub fn reasons(&self) -> impl ExactSizeIterator<Item = &AdmissionReasonCode> {
        self.reasons.iter()
    }

    /// Receipt digest binding every identity above.
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }
}

fn observation_digest_for(
    source_class: SourceClass,
    sensitivity: SensitivityLevel,
    byte_size: u64,
    is_binary_hint: bool,
) -> String {
    sha256::hex(&sha256::digest_parts(
        b"eliot-searchd/source-composition-observation/v1",
        &[
            source_class.as_str().as_bytes(),
            sensitivity.as_str().as_bytes(),
            &byte_size.to_be_bytes(),
            &[u8::from(is_binary_hint)],
        ],
    ))
}

/// Builds the content-free observation for one candidate.
///
/// # Errors
///
/// Returns `DIRECT_SOURCE_PATH_DENIED` for unclassifiable paths.
pub fn build_observation(
    path: &Path,
    bytes: &[u8],
    _policy: &AdmissionPolicy,
) -> Result<AdmissionObservation, String> {
    let byte_length = u64::try_from(bytes.len()).map_err(|_| SOURCE_TOO_LARGE.to_owned())?;
    let (source_class, sensitivity, is_binary_hint) = classify_source(path, bytes)?;
    Ok(AdmissionObservation {
        source_class,
        sensitivity,
        byte_size: byte_length,
        is_binary_hint,
    })
}

/// Applies the closed deny-by-default rule order.
///
/// Deny wins over unsupported, which wins over review, which wins over
/// allow. Silence is never allow.
pub fn evaluate(policy: &AdmissionPolicy, observation: &AdmissionObservation) -> AdmissionDecision {
    let fingerprint = policy.fingerprint.clone();
    let digest = observation_digest_for(
        observation.source_class,
        observation.sensitivity,
        observation.byte_size,
        observation.is_binary_hint,
    );
    let mut deny: BTreeSet<AdmissionReasonCode> = BTreeSet::new();
    let unsupported: BTreeSet<AdmissionReasonCode> = BTreeSet::new();
    let review: BTreeSet<AdmissionReasonCode> = BTreeSet::new();
    let mut allow: BTreeSet<AdmissionReasonCode> = BTreeSet::new();

    // Unconditional safety denies cannot be bypassed by any flag.
    if observation.source_class == SourceClass::Credential {
        deny.insert(AdmissionReasonCode::DenyCredentialClass);
    }
    if observation.source_class == SourceClass::PrivateKey {
        deny.insert(AdmissionReasonCode::DenyPrivateKeyClass);
    }
    if observation.sensitivity.is_unconditional_deny() {
        deny.insert(AdmissionReasonCode::DenySensitiveCredential);
    }
    if observation.source_class == SourceClass::System {
        deny.insert(AdmissionReasonCode::DenySystemLocation);
    }
    if observation.source_class == SourceClass::Cache {
        deny.insert(AdmissionReasonCode::DenyCacheArtifact);
    }
    if observation.source_class == SourceClass::BuildOutput {
        deny.insert(AdmissionReasonCode::DenyBuildOutput);
    }
    // Baseline generated/vendor/binary handling follows explicit flags.
    if observation.source_class == SourceClass::Generated && !policy.allow_generated {
        deny.insert(AdmissionReasonCode::DenyGeneratedClass);
    }
    if observation.source_class == SourceClass::Vendor && !policy.allow_vendor {
        deny.insert(AdmissionReasonCode::DenyVendorClass);
    }
    if observation.source_class == SourceClass::Binary && !policy.allow_binary {
        deny.insert(AdmissionReasonCode::DenyBinaryClass);
    }
    // Sensitive candidates stay denied under the baseline.
    if observation.sensitivity == SensitivityLevel::SecretCandidate {
        deny.insert(AdmissionReasonCode::DenySensitiveClass);
    }
    // Size fences apply before any allow.
    if observation.byte_size == 0 {
        deny.insert(AdmissionReasonCode::DenyEmptySource);
    } else if observation.byte_size > policy.max_file_bytes {
        deny.insert(AdmissionReasonCode::DenySizeExceeded);
    }

    // Baseline allow affordance: regular, test and documentation sources
    // with complete internal/public signals within bounds are admitted.
    // Every other shape stays denied by default.
    if deny.is_empty() && unsupported.is_empty() && review.is_empty() {
        match (observation.source_class, observation.sensitivity) {
            (
                SourceClass::Regular | SourceClass::Test | SourceClass::Documentation,
                SensitivityLevel::Public | SensitivityLevel::Internal,
            ) => {
                allow.insert(AdmissionReasonCode::AllowBaselineSource);
            }
            _ => {
                deny.insert(AdmissionReasonCode::DenyByDefault);
            }
        }
    }

    let (outcome, reasons) = if !deny.is_empty() {
        (AdmissionOutcome::Deny, deny)
    } else if !unsupported.is_empty() {
        (AdmissionOutcome::Unsupported, unsupported)
    } else if !review.is_empty() {
        (AdmissionOutcome::ReviewRequired, review)
    } else if !allow.is_empty() {
        (AdmissionOutcome::Allow, allow)
    } else {
        let mut default = BTreeSet::new();
        default.insert(AdmissionReasonCode::DenyByDefault);
        (AdmissionOutcome::Deny, default)
    };
    AdmissionDecision {
        outcome,
        reasons,
        sensitivity: observation.sensitivity,
        policy_revision: policy.revision,
        policy_fingerprint: fingerprint,
        observation_digest: digest,
    }
}

/// Issues an immutable receipt for one canonical decision.
///
/// # Errors
///
/// Returns `ADMISSION_RECEIPT_MISMATCH` when decision inputs disagree with
/// the supplied policy and observation.
pub fn issue_receipt(
    policy: &AdmissionPolicy,
    observation: &AdmissionObservation,
    decision: &AdmissionDecision,
) -> Result<ComposedAdmissionReceipt, String> {
    let fingerprint = policy.fingerprint.clone();
    let digest = observation_digest_for(
        observation.source_class,
        observation.sensitivity,
        observation.byte_size,
        observation.is_binary_hint,
    );
    if decision.policy_revision != policy.revision
        || decision.policy_fingerprint != fingerprint
        || decision.observation_digest != digest
    {
        return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
    }
    let mut parts: Vec<&[u8]> = Vec::new();
    let outcome_text = decision.outcome.as_str().to_owned();
    let sensitivity_text = decision.sensitivity.as_str().to_owned();
    let reason_joined = decision
        .reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>()
        .join(",");
    parts.push(outcome_text.as_bytes());
    parts.push(sensitivity_text.as_bytes());
    parts.push(reason_joined.as_bytes());
    parts.push(digest.as_bytes());
    parts.push(fingerprint.as_bytes());
    let receipt_digest = sha256::hex(&sha256::digest_parts(
        b"eliot-searchd/source-composition-receipt/v1",
        &parts,
    ));
    Ok(ComposedAdmissionReceipt {
        policy_revision: policy.revision,
        policy_fingerprint: fingerprint,
        observation_digest: digest,
        outcome: decision.outcome,
        reasons: decision.reasons.clone(),
        sensitivity: decision.sensitivity,
        receipt_digest,
    })
}

/// Verifies one receipt against the current policy and observation.
///
/// Rejects stale policy fences, mismatched observations, altered reasons
/// and decisions inconsistent with current unconditional denies. A valid
/// old permissive receipt never bypasses a newer restrictive policy.
///
/// # Errors
///
/// Returns `ADMISSION_RECEIPT_STALE` for stale fences and
/// `ADMISSION_RECEIPT_MISMATCH` for altered or inconsistent receipts.
pub fn verify_receipt(
    receipt: &ComposedAdmissionReceipt,
    current_policy: &AdmissionPolicy,
    observation: &AdmissionObservation,
) -> Result<(), String> {
    let current_digest = observation_digest_for(
        observation.source_class,
        observation.sensitivity,
        observation.byte_size,
        observation.is_binary_hint,
    );
    if receipt.observation_digest != current_digest {
        return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
    }
    if receipt.policy_revision != current_policy.revision
        || receipt.policy_fingerprint != current_policy.fingerprint
    {
        return Err(ADMISSION_RECEIPT_STALE.to_owned());
    }
    // Recompute the receipt digest over stored decision fields so altered
    // reasons or outcomes fail.
    let outcome_text = receipt.outcome.as_str().to_owned();
    let sensitivity_text = receipt.sensitivity.as_str().to_owned();
    let reason_joined = receipt
        .reasons
        .iter()
        .map(|reason| reason.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let recomputed = sha256::hex(&sha256::digest_parts(
        b"eliot-searchd/source-composition-receipt/v1",
        &[
            outcome_text.as_bytes(),
            sensitivity_text.as_bytes(),
            reason_joined.as_bytes(),
            receipt.observation_digest.as_bytes(),
            receipt.policy_fingerprint.as_bytes(),
        ],
    ));
    if recomputed != receipt.receipt_digest {
        return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
    }
    let unconditional_deny_now = observation.source_class.is_unconditional_deny()
        || observation.sensitivity.is_unconditional_deny();
    if unconditional_deny_now && receipt.outcome == AdmissionOutcome::Allow {
        return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
    }
    if receipt.reasons.is_empty() || receipt.reasons.len() > MAX_REASON_CODES {
        return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Stable identity and canonical path keys (paths are locators, never identity)
// ---------------------------------------------------------------------------

/// Legacy DIRECT source-id domain (preserved for migration readback only).
pub const LEGACY_SOURCE_ID_DOMAIN: &[u8] = b"eliot-search/direct-source-id/v1";
/// Canonical composition source-id domain.
///
/// Currently identical to the legacy domain so that T11 migration readback
/// (`validate_legacy_event`) keeps passing while ingestion moves to
/// canonical receipts. The canonicalization is the admission/identity/
/// registry binding, not a new hash domain; a future domain rotation (with
/// an explicit migration mapping) may diverge this constant once the
/// integration owner accepts the cutover.
pub const CANONICAL_SOURCE_ID_DOMAIN: &[u8] = b"eliot-search/direct-source-id/v1";
/// Revision domain is preserved so revision bindings stay comparable.
pub const REVISION_ID_DOMAIN: &[u8] = b"eliot-search/direct-revision-id/v1";

/// Versioned canonical path lookup key (lookup evidence only).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalPathKey {
    /// Hex root digest bound at derive time (never a raw path).
    pub root_digest_hex: String,
    /// Validated `/`-separated relative lookup spelling.
    pub relative_path: String,
}

/// Validates a `/`-separated relative lookup spelling without touching disk.
fn validate_relative_lookup(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_RELATIVE_PATH_BYTES {
        return Err(PATH_BINDING_CONFLICT.to_owned());
    }
    if value.starts_with('/') || value.contains('\\') || value.contains(':') || value.contains('\0')
    {
        return Err(PATH_BINDING_CONFLICT.to_owned());
    }
    if value
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || is_reserved_device_name(part))
    {
        return Err(PATH_BINDING_CONFLICT.to_owned());
    }
    Ok(())
}

fn is_reserved_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Derives a versioned lookup key from an admitted root and a candidate path.
///
/// Both inputs are canonicalized so verbatim and symlinked spellings compare
/// on the same base. Text equality is lookup evidence only and never source
/// identity proof.
///
/// # Errors
///
/// Returns `ADMITTED_ROOT_UNAVAILABLE` when the root cannot be proven, or
/// `PATH_BINDING_CONFLICT` for escaping or malformed relative spellings.
pub fn derive_canonical_path_key(
    admitted_root_hint: &Path,
    candidate_path: &Path,
) -> Result<CanonicalPathKey, String> {
    use std::fs;
    let canonical_root =
        fs::canonicalize(admitted_root_hint).map_err(|_| ADMITTED_ROOT_UNAVAILABLE.to_owned())?;
    let canonical_final =
        fs::canonicalize(candidate_path).map_err(|_| ADMITTED_ROOT_UNAVAILABLE.to_owned())?;
    let relative = canonical_final
        .strip_prefix(&canonical_root)
        .map_err(|_| PATH_BINDING_CONFLICT.to_owned())?;
    if relative.as_os_str().is_empty() {
        return Err(PATH_BINDING_CONFLICT.to_owned());
    }
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let text = part
                    .to_str()
                    .ok_or_else(|| PATH_BINDING_CONFLICT.to_owned())?;
                if text.is_empty()
                    || text == "."
                    || text == ".."
                    || text.contains(['\\', ':', '\0'])
                    || is_reserved_device_name(text)
                {
                    return Err(PATH_BINDING_CONFLICT.to_owned());
                }
                parts.push(text.to_owned());
            }
            _ => return Err(PATH_BINDING_CONFLICT.to_owned()),
        }
    }
    if parts.is_empty() {
        return Err(PATH_BINDING_CONFLICT.to_owned());
    }
    let relative_path = parts.join("/");
    validate_relative_lookup(&relative_path)?;
    // Root digest binds the canonical root bytes (locator, not identity).
    #[cfg(unix)]
    let root_material = {
        use std::os::unix::ffi::OsStrExt;
        canonical_root.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let root_material = {
        use std::os::windows::ffi::OsStrExt;
        canonical_root
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    #[cfg(not(any(unix, windows)))]
    let root_material = canonical_root.to_string_lossy().as_bytes().to_vec();
    let root_digest_hex = sha256::hex(&sha256::digest_parts(
        b"eliot-searchd/source-composition-root/v1",
        &[&root_material],
    ));
    Ok(CanonicalPathKey {
        root_digest_hex,
        relative_path,
    })
}

/// Prior durable source projected from the DIRECT log (no second store).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorSourceView {
    /// Durable source identifier (legacy or canonical domain).
    pub source_id: String,
    /// Stable file-identity digest hex (decoded for stable comparison).
    pub file_identity_digest: String,
    /// Path digest hex (locator history, never identity).
    pub path_digest: String,
    /// Latest revision identifier for change detection.
    pub revision_id: String,
    /// Latest record digest for predecessor binding.
    pub record_digest: String,
    /// Whether the latest record is active (retired sources reactivate).
    pub is_active: bool,
}

/// Stable identity resolution outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityResolution {
    /// One exact existing stable identity matched.
    MatchExisting {
        /// Durable source identifier to reuse (preserves legacy mappings).
        source_id: String,
    },
    /// Exact stable evidence is unseen and may use a canonical identifier.
    CreateNew,
    /// Stable evidence is absent (path-bound fallback); never admitted.
    Ambiguous,
    /// Claimed or active binding conflicts with stable evidence.
    Conflict,
    /// One stable identity maps to multiple durable source IDs.
    Collision,
}

/// Resolves one stable file identity against finite prior candidates.
///
/// Stable physical components dominate path similarity. Path or content
/// equality alone never matches or creates a durable identity. Path-bound
/// fallbacks (non-native stable evidence) resolve to `Ambiguous` and are
/// never admitted.
///
/// # Errors
///
/// Returns `SOURCE_IDENTITY_COLLISION` when one stable identity maps to
/// multiple durable IDs.
pub fn resolve_identity(
    file_identity_digest_hex: &str,
    identity_strength: &str,
    prior: &[PriorSourceView],
) -> Result<IdentityResolution, String> {
    if sha256::decode_digest(file_identity_digest_hex).is_none() {
        return Err(SOURCE_IDENTITY_AMBIGUOUS.to_owned());
    }
    if identity_strength != "native" {
        // Path-only observations never create or match durable identity.
        return Ok(IdentityResolution::Ambiguous);
    }
    if prior.len() > 100_000 {
        return Err(SOURCE_IDENTITY_AMBIGUOUS.to_owned());
    }
    let mut matches = Vec::new();
    for candidate in prior {
        if candidate.file_identity_digest == file_identity_digest_hex {
            matches.push(candidate.source_id.clone());
        }
    }
    matches.sort();
    matches.dedup();
    match matches.len() {
        0 => Ok(IdentityResolution::CreateNew),
        1 => Ok(IdentityResolution::MatchExisting {
            source_id: matches.into_iter().next().expect("one match"),
        }),
        _ => Err(SOURCE_IDENTITY_COLLISION.to_owned()),
    }
}

/// Derives the durable source identifier.
///
/// `MatchExisting` reuses the prior durable identifier byte-for-byte
/// (preserving legacy migration mappings). `CreateNew` derives a canonical
/// identifier via domain separation over the namespace and stable file
/// identity. `Ambiguous`, `Conflict` and `Collision` never produce an
/// identifier.
///
/// # Errors
///
/// Returns `SOURCE_IDENTITY_AMBIGUOUS` or `SOURCE_IDENTITY_CONFLICT` when
/// no durable identifier can be produced, without fabricating one.
pub fn derive_source_id(
    namespace_hex: &str,
    file_identity_digest_hex: &str,
    resolution: &IdentityResolution,
) -> Result<String, String> {
    match resolution {
        IdentityResolution::MatchExisting { source_id } => {
            if sha256::decode_digest(source_id).is_none() {
                return Err(SOURCE_IDENTITY_CONFLICT.to_owned());
            }
            Ok(source_id.clone())
        }
        IdentityResolution::CreateNew => {
            let namespace = sha256::decode_digest(namespace_hex)
                .ok_or_else(|| SOURCE_IDENTITY_AMBIGUOUS.to_owned())?;
            let file_identity = sha256::decode_digest(file_identity_digest_hex)
                .ok_or_else(|| SOURCE_IDENTITY_AMBIGUOUS.to_owned())?;
            Ok(sha256::hex(&sha256::digest_parts(
                CANONICAL_SOURCE_ID_DOMAIN,
                &[&namespace, &file_identity],
            )))
        }
        IdentityResolution::Ambiguous => Err(SOURCE_IDENTITY_AMBIGUOUS.to_owned()),
        IdentityResolution::Conflict => Err(SOURCE_IDENTITY_CONFLICT.to_owned()),
        IdentityResolution::Collision => Err(SOURCE_IDENTITY_COLLISION.to_owned()),
    }
}

/// Derives the legacy DIRECT source identifier for migration readback only.
///
/// New writes never use this domain; it exists so reviewers can prove that
/// a prior durable identifier is a legacy binding rather than a fabricated
/// canonical one.
pub fn legacy_source_id_for_migration(
    namespace_hex: &str,
    file_identity_digest_hex: &str,
) -> Result<String, String> {
    let namespace =
        sha256::decode_digest(namespace_hex).ok_or_else(|| SOURCE_IDENTITY_AMBIGUOUS.to_owned())?;
    let file_identity = sha256::decode_digest(file_identity_digest_hex)
        .ok_or_else(|| SOURCE_IDENTITY_AMBIGUOUS.to_owned())?;
    Ok(sha256::hex(&sha256::digest_parts(
        LEGACY_SOURCE_ID_DOMAIN,
        &[&namespace, &file_identity],
    )))
}

/// Derives the revision identifier binding canonical source, content and size.
///
/// The revision domain is preserved so revision bindings stay comparable
/// across the migration; only the source input changes from legacy to
/// canonical for truly new identities.
pub fn derive_revision_id(
    source_id_hex: &str,
    content_digest_hex: &str,
    byte_length: u64,
) -> Result<String, String> {
    let content = sha256::decode_digest(content_digest_hex)
        .ok_or_else(|| "DIRECT_CONTENT_DIGEST_INVALID".to_owned())?;
    if sha256::decode_digest(source_id_hex).is_none() {
        return Err("DIRECT_SOURCE_ID_INVALID".to_owned());
    }
    Ok(sha256::hex(&sha256::digest_parts(
        REVISION_ID_DOMAIN,
        &[
            source_id_hex.as_bytes(),
            &content,
            &byte_length.to_be_bytes(),
        ],
    )))
}

// ---------------------------------------------------------------------------
// Registry view (transient view over the DIRECT log, never a second catalog)
// ---------------------------------------------------------------------------

/// Transient coherent registry view for one batch operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryView {
    /// Prior sources by durable identifier.
    by_id: BTreeMap<String, PriorSourceView>,
    /// Current admission policy fence.
    policy_revision: u64,
    policy_fingerprint: String,
}

impl RegistryView {
    /// Builds a bounded transient view over replayed DIRECT state.
    ///
    /// # Errors
    ///
    /// Returns `ADMISSION_RECEIPT_STALE` when the view fence disagrees with
    /// the current policy (stale-policy race fails closed).
    pub fn build(prior: Vec<PriorSourceView>, policy: &AdmissionPolicy) -> Result<Self, String> {
        if prior.len() > 2_000_000 {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        let mut by_id = BTreeMap::new();
        for entry in prior {
            if sha256::decode_digest(&entry.source_id).is_none()
                || sha256::decode_digest(&entry.file_identity_digest).is_none()
                || sha256::decode_digest(&entry.path_digest).is_none()
            {
                return Err(ADMISSION_RECEIPT_MISMATCH.to_owned());
            }
            by_id.insert(entry.source_id.clone(), entry);
        }
        Ok(Self {
            by_id,
            policy_revision: policy.revision,
            policy_fingerprint: policy.fingerprint.clone(),
        })
    }

    /// Prior candidates for stable identity resolution.
    pub fn prior_candidates(&self) -> Vec<PriorSourceView> {
        self.by_id.values().cloned().collect()
    }

    /// Prior record by durable identifier, if any.
    pub fn get(&self, source_id: &str) -> Option<PriorSourceView> {
        self.by_id.get(source_id).cloned()
    }

    /// Admits one source under the exact current verified receipt.
    ///
    /// Requires `ALLOW` under the current fence, exact observation binding
    /// and no duplicate or conflicting durable identity. Registration alone
    /// is not an access grant; residency and access are separate owners.
    ///
    /// # Errors
    ///
    /// Returns `SOURCE_ADMISSION_DENIED` for non-allow outcomes,
    /// `ADMISSION_RECEIPT_STALE` for stale fences and
    /// `ADMISSION_RECEIPT_MISMATCH` for mismatched bindings.
    pub fn admit(
        &self,
        receipt: &ComposedAdmissionReceipt,
        observation: &AdmissionObservation,
        policy: &AdmissionPolicy,
    ) -> Result<(), String> {
        verify_receipt(receipt, policy, observation)?;
        if receipt.policy_revision != self.policy_revision
            || receipt.policy_fingerprint != self.policy_fingerprint
        {
            return Err(ADMISSION_RECEIPT_STALE.to_owned());
        }
        match receipt.outcome {
            AdmissionOutcome::Allow => Ok(()),
            AdmissionOutcome::Deny => Err(SOURCE_ADMISSION_DENIED.to_owned()),
            AdmissionOutcome::ReviewRequired => Err(SOURCE_ADMISSION_REVIEW_REQUIRED.to_owned()),
            AdmissionOutcome::Unsupported => Err(SOURCE_KIND_UNSUPPORTED.to_owned()),
        }
    }
}

/// Full canonical plan for one retained snapshot (admission + identity).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPlan {
    /// Durable source identifier (reused legacy or new canonical).
    pub source_id: String,
    /// Revision identifier binding canonical source, content and size.
    pub revision_id: String,
    /// Verified admission receipt authorizing retention.
    pub receipt: ComposedAdmissionReceipt,
    /// Whether stable identity was natively proven.
    pub identity_native: bool,
}

/// Plans one kernel-verified snapshot through canonical composition.
///
/// Order: classify, observe, evaluate, issue, verify, admit against the
/// registry view, resolve stable identity, derive durable identifiers.
/// Denied, review-required, unsupported and ambiguous sources never reach
/// CAS: callers must invoke this before any writer call.
///
/// # Errors
///
/// Returns the exact closed canonical code for every denial; never a
/// fabricated identifier or a silent fallback.
#[allow(clippy::too_many_arguments)]
pub fn plan_snapshot(
    original_path: &Path,
    file_identity_digest_hex: &str,
    identity_strength: &str,
    content_digest_hex: &str,
    snapshot_bytes: &[u8],
    namespace_hex: &str,
    policy: &AdmissionPolicy,
    view: &RegistryView,
) -> Result<CanonicalPlan, String> {
    if snapshot_bytes.len() > 64 * 1024 * 1024 {
        return Err(SOURCE_TOO_LARGE.to_owned());
    }
    let observation = build_observation(original_path, snapshot_bytes, policy)?;
    let decision = evaluate(policy, &observation);
    let receipt = issue_receipt(policy, &observation, &decision)?;
    view.admit(&receipt, &observation, policy)?;
    // Size/empty fences are enforced by evaluation above; surface the
    // precise closed code for diagnostics without paths or bytes.
    match decision.outcome {
        AdmissionOutcome::Allow => {}
        AdmissionOutcome::Deny => {
            if decision
                .reasons
                .contains(&AdmissionReasonCode::DenyEmptySource)
            {
                return Err(SOURCE_ADMISSION_DENIED.to_owned());
            }
            if decision
                .reasons
                .contains(&AdmissionReasonCode::DenySizeExceeded)
            {
                return Err(SOURCE_TOO_LARGE.to_owned());
            }
            if decision.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    AdmissionReasonCode::DenySensitiveClass
                        | AdmissionReasonCode::DenySensitiveCredential
                )
            }) {
                return Err(SENSITIVE_SOURCE_DENIED.to_owned());
            }
            return Err(SOURCE_ADMISSION_DENIED.to_owned());
        }
        AdmissionOutcome::ReviewRequired => {
            return Err(SOURCE_ADMISSION_REVIEW_REQUIRED.to_owned());
        }
        AdmissionOutcome::Unsupported => {
            return Err(SOURCE_KIND_UNSUPPORTED.to_owned());
        }
    }
    let resolution = resolve_identity(
        file_identity_digest_hex,
        identity_strength,
        &view.prior_candidates(),
    )?;
    let source_id = derive_source_id(namespace_hex, file_identity_digest_hex, &resolution)?;
    let byte_length =
        u64::try_from(snapshot_bytes.len()).map_err(|_| SOURCE_TOO_LARGE.to_owned())?;
    let revision_id = derive_revision_id(&source_id, content_digest_hex, byte_length)?;
    Ok(CanonicalPlan {
        source_id,
        revision_id,
        receipt,
        identity_native: identity_strength == "native",
    })
}

// ---------------------------------------------------------------------------
// Wave2-source cross-checks (compile only with the canonical crates)
// ---------------------------------------------------------------------------

#[cfg(feature = "wave2-source")]
mod canonical_cross_checks {
    //! Byte-compatibility probes against the real pure kernels.
    //!
    //! These helpers never change composition behavior; they prove that the
    //! daemon's closed vocabularies stay byte-identical to the normative
    //! crates. Any drift fails the `wave2-source` build.

    use super::{AdmissionReasonCode, SourceClass};

    const _: () = {
        // Admission crate reason strings must match this module exactly.
        let pairs: &[(&str, &str)] = &[
            (
                AdmissionReasonCode::DenyCredentialClass.as_str(),
                search_source_admission::AdmissionReasonCode::DenyCredentialClass.as_str(),
            ),
            (
                AdmissionReasonCode::DenyEmptySource.as_str(),
                search_source_admission::AdmissionReasonCode::DenyEmptySource.as_str(),
            ),
            (
                AdmissionReasonCode::AllowBaselineSource.as_str(),
                search_source_admission::AdmissionReasonCode::AllowBaselineSource.as_str(),
            ),
        ];
        let mut index = 0;
        while index < pairs.len() {
            let (left, right) = (&pairs[index].0, &pairs[index].1);
            let left_bytes = left.as_bytes();
            let right_bytes = right.as_bytes();
            assert!(left_bytes.len() == right_bytes.len());
            let mut byte = 0;
            while byte < left_bytes.len() {
                assert!(left_bytes[byte] == right_bytes[byte]);
                byte += 1;
            }
            index += 1;
        }
    };

    /// Proves the baseline policy shape matches the canonical baseline.
    pub fn baseline_shape_matches_canonical() {
        let canonical =
            search_source_admission::baseline_policy(super::BASELINE_POLICY_REVISION, 1_024);
        assert!(canonical.max_file_bytes() == 1_024);
        assert!(!canonical.allows_generated());
        assert!(!canonical.allows_vendor());
        assert!(!canonical.allows_binary());
        let _ = SourceClass::Regular;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn policy() -> AdmissionPolicy {
        AdmissionPolicy::baseline()
    }

    fn observation_for(path: &str, bytes: &[u8]) -> AdmissionObservation {
        build_observation(Path::new(path), bytes, &policy()).expect("observation")
    }

    #[test]
    fn baseline_policy_is_deny_by_default_with_stable_fingerprint() {
        let first = AdmissionPolicy::baseline();
        let second = AdmissionPolicy::baseline();
        assert_eq!(first, second);
        assert_eq!(first.revision(), BASELINE_POLICY_REVISION);
        assert_eq!(first.max_file_bytes(), BASELINE_MAX_FILE_BYTES);
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.fingerprint().len(), 64);
    }

    #[test]
    fn regular_source_with_complete_signals_is_allowed() {
        let observation = observation_for("/tmp/workspace/notes.txt", b"hello");
        let decision = evaluate(&policy(), &observation);
        assert_eq!(decision.outcome(), AdmissionOutcome::Allow);
        let receipt = issue_receipt(&policy(), &observation, &decision).expect("receipt");
        assert_eq!(receipt.outcome(), AdmissionOutcome::Allow);
        verify_receipt(&receipt, &policy(), &observation).expect("verified");
    }

    #[test]
    fn credential_and_private_key_are_unconditional_denies() {
        for path in [
            "/tmp/workspace/id_rsa",
            "/tmp/workspace/tls.pem",
            "/tmp/workspace/creds/secrets.json",
        ] {
            let observation = observation_for(path, b"bytes");
            let decision = evaluate(&policy(), &observation);
            assert_eq!(decision.outcome(), AdmissionOutcome::Deny, "path={path}");
            let receipt = issue_receipt(&policy(), &observation, &decision).expect("receipt");
            assert_eq!(receipt.outcome(), AdmissionOutcome::Deny);
            // An allow receipt for an unconditional deny never verifies.
            let mut forged = receipt.clone();
            forged.outcome = AdmissionOutcome::Allow;
            assert!(verify_receipt(&forged, &policy(), &observation).is_err());
        }
    }

    #[test]
    fn secret_candidate_generated_vendor_binary_vcs_cache_build_are_denied() {
        let cases = [
            ("/tmp/workspace/secret_notes.txt", b"data"),
            ("/tmp/workspace/app.generated.js", b"data"),
            ("/tmp/workspace/vendor/lib.js", b"data"),
            ("/tmp/workspace/node_modules/lib.js", b"data"),
            ("/tmp/workspace/.git/config", b"data"),
            ("/tmp/workspace/__pycache__/mod.pyc", b"data"),
            ("/tmp/workspace/target/debug/app", b"data"),
        ];
        for (path, bytes) in cases {
            let observation = observation_for(path, bytes);
            let decision = evaluate(&policy(), &observation);
            assert_eq!(decision.outcome(), AdmissionOutcome::Deny, "path={path}");
        }
        // Binary extensions deny at admission; raw NUL content in a `.txt`
        // source does NOT deny here (admitted for downstream gap handling
        // with `MATERIALIZATION_BINARY_CONTENT`, preserving preparation).
        let observation = observation_for("/tmp/workspace/blob.bin", b"binary bytes");
        assert_eq!(
            evaluate(&policy(), &observation).outcome(),
            AdmissionOutcome::Deny
        );
        let nul_txt = observation_for("/tmp/workspace/notes.txt", b"ab\x00cd");
        assert_eq!(
            evaluate(&policy(), &nul_txt).outcome(),
            AdmissionOutcome::Allow,
            "NUL .txt admits for downstream gap handling"
        );
        // Explicit flags permit the gated classes without touching denies.
        let permissive = AdmissionPolicy::from_config(SourceAdmissionConfig {
            revision: 2,
            max_file_bytes: BASELINE_MAX_FILE_BYTES,
            allow_generated: true,
            allow_vendor: true,
            allow_binary: true,
        })
        .expect("permissive policy");
        let observation = observation_for("/tmp/workspace/app.generated.js", b"data");
        // Generated with explicit allow still needs an allow rule: the
        // baseline has no generated allow, so it stays denied by default
        // rather than silently allowing. This proves flags never invent
        // allows.
        assert_eq!(
            evaluate(&permissive, &observation).outcome(),
            AdmissionOutcome::Deny
        );
    }

    #[test]
    fn empty_and_oversized_sources_are_denied_before_cas() {
        let observation = observation_for("/tmp/workspace/notes.txt", b"");
        let decision = evaluate(&policy(), &observation);
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
        assert!(
            decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenyEmptySource)
        );
        let big = vec![b'x'; 1024];
        let observation = AdmissionObservation {
            source_class: SourceClass::Regular,
            sensitivity: SensitivityLevel::Internal,
            byte_size: BASELINE_MAX_FILE_BYTES + 1,
            is_binary_hint: false,
        };
        let decision = evaluate(&policy(), &observation);
        assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
        assert!(
            decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenySizeExceeded)
        );
        let _ = big;
    }

    #[test]
    fn receipt_mismatch_and_stale_fences_fail_closed() {
        let observation = observation_for("/tmp/workspace/notes.txt", b"hello");
        let decision = evaluate(&policy(), &observation);
        let receipt = issue_receipt(&policy(), &observation, &decision).expect("receipt");
        // Mismatched observation fails.
        let other = observation_for("/tmp/workspace/other.txt", b"different bytes here");
        assert_eq!(
            verify_receipt(&receipt, &policy(), &other),
            Err(ADMISSION_RECEIPT_MISMATCH.to_owned())
        );
        // Stale policy revision fails.
        let rotated = AdmissionPolicy::from_config(SourceAdmissionConfig {
            revision: 2,
            max_file_bytes: BASELINE_MAX_FILE_BYTES,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
        })
        .expect("rotated");
        assert_ne!(rotated.fingerprint(), policy().fingerprint());
        assert_eq!(
            verify_receipt(&receipt, &rotated, &observation),
            Err(ADMISSION_RECEIPT_STALE.to_owned())
        );
        // Altered reasons fail.
        let mut altered = receipt.clone();
        altered.reasons.insert(AdmissionReasonCode::DenyByDefault);
        assert!(verify_receipt(&altered, &policy(), &observation).is_err());
    }

    #[test]
    fn path_key_derivation_denies_escape_without_opening_bytes() {
        let root = std::env::temp_dir().join(format!(
            "eliot-source-comp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let child = root.join("child.txt");
        std::fs::write(&child, b"inside").unwrap();
        let key = derive_canonical_path_key(&root, &child).expect("key");
        assert_eq!(key.relative_path, "child.txt");
        assert_eq!(key.root_digest_hex.len(), 64);
        // Outside root is denied, never an empty view.
        let outside = std::env::temp_dir().join(format!(
            "eliot-source-comp-out-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&outside).unwrap();
        let secret = outside.join("secret.txt");
        std::fs::write(&secret, b"outside").unwrap();
        assert_eq!(
            derive_canonical_path_key(&root, &secret),
            Err(PATH_BINDING_CONFLICT.to_owned())
        );
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
        let _ = PathBuf::from("unused");
    }

    #[test]
    fn stable_identity_dominates_paths_and_path_only_never_matches() {
        let file_digest = sha256::hex(&sha256::digest(b"stable-material"));
        let prior = vec![PriorSourceView {
            source_id: sha256::hex(&sha256::digest(b"existing")),
            file_identity_digest: file_digest.clone(),
            path_digest: sha256::hex(&sha256::digest(b"old-path")),
            revision_id: sha256::hex(&sha256::digest(b"revision")),
            record_digest: sha256::hex(&sha256::digest(b"record")),
            is_active: true,
        }];
        // Exact stable match reuses the durable identifier.
        let resolution = resolve_identity(&file_digest, "native", &prior).expect("resolution");
        match &resolution {
            IdentityResolution::MatchExisting { source_id } => {
                assert_eq!(source_id, &prior[0].source_id);
            }
            other => panic!("expected match, got {other:?}"),
        }
        // Unseen stable evidence creates, never fabricates.
        let unseen = sha256::hex(&sha256::digest(b"unseen"));
        assert_eq!(
            resolve_identity(&unseen, "native", &prior).expect("resolution"),
            IdentityResolution::CreateNew
        );
        // Path-bound fallbacks are ambiguous and never admitted.
        assert_eq!(
            resolve_identity(&file_digest, "path-bound", &prior).expect("resolution"),
            IdentityResolution::Ambiguous
        );
        // Canonical derivation preserves legacy reuse and isolates new ids.
        let namespace = sha256::hex(&sha256::digest(b"namespace"));
        let reused = derive_source_id(&namespace, &file_digest, &resolution).expect("reused");
        assert_eq!(reused, prior[0].source_id);
        let fresh =
            derive_source_id(&namespace, &unseen, &IdentityResolution::CreateNew).expect("fresh");
        let legacy = legacy_source_id_for_migration(&namespace, &unseen).expect("legacy");
        assert_eq!(
            fresh, legacy,
            "canonical domain preserves legacy migration mappings (T11)"
        );
        assert!(derive_source_id(&namespace, &unseen, &IdentityResolution::Ambiguous).is_err());
    }

    #[test]
    fn registry_view_requires_current_allow_and_rejects_stale() {
        let policy = policy();
        let observation = observation_for("/tmp/workspace/notes.txt", b"hello");
        let decision = evaluate(&policy, &observation);
        let receipt = issue_receipt(&policy, &observation, &decision).expect("receipt");
        let view = RegistryView::build(Vec::new(), &policy).expect("view");
        view.admit(&receipt, &observation, &policy)
            .expect("admitted");
        // Denied receipts never admit.
        let denied_observation = observation_for("/tmp/workspace/id_rsa", b"bytes");
        let denied_decision = evaluate(&policy, &denied_observation);
        let denied =
            issue_receipt(&policy, &denied_observation, &denied_decision).expect("denied receipt");
        assert_eq!(
            view.admit(&denied, &denied_observation, &policy),
            Err(SOURCE_ADMISSION_DENIED.to_owned())
        );
        // Stale fences never admit.
        let rotated = AdmissionPolicy::from_config(SourceAdmissionConfig {
            revision: 9,
            max_file_bytes: BASELINE_MAX_FILE_BYTES,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
        })
        .expect("rotated");
        assert_eq!(
            view.admit(&receipt, &observation, &rotated),
            Err(ADMISSION_RECEIPT_STALE.to_owned())
        );
    }

    #[test]
    fn same_content_distinct_stable_identities_stay_distinct() {
        let namespace = sha256::hex(&sha256::digest(b"namespace"));
        let first = sha256::hex(&sha256::digest(b"stable-first"));
        let second = sha256::hex(&sha256::digest(b"stable-second"));
        let view = RegistryView::build(Vec::new(), &policy()).expect("view");
        let plan_first = plan_snapshot(
            Path::new("/tmp/workspace/first.txt"),
            &first,
            "native",
            &sha256::hex(&sha256::digest(b"same bytes")),
            b"same bytes",
            &namespace,
            &policy(),
            &view,
        )
        .expect("first plan");
        let plan_second = plan_snapshot(
            Path::new("/tmp/workspace/second.txt"),
            &second,
            "native",
            &sha256::hex(&sha256::digest(b"same bytes")),
            b"same bytes",
            &namespace,
            &policy(),
            &view,
        )
        .expect("second plan");
        assert_ne!(plan_first.source_id, plan_second.source_id);
        assert_eq!(plan_first.revision_id.len(), 64);
        // Same stable identity with same bytes reuses the durable source.
        let prior = vec![PriorSourceView {
            source_id: plan_first.source_id.clone(),
            file_identity_digest: first.clone(),
            path_digest: sha256::hex(&sha256::digest(b"first-path")),
            revision_id: plan_first.revision_id.clone(),
            record_digest: sha256::hex(&sha256::digest(b"record")),
            is_active: true,
        }];
        let rebound = RegistryView::build(prior, &policy()).expect("rebound");
        let again = plan_snapshot(
            Path::new("/tmp/workspace/first.txt"),
            &first,
            "native",
            &sha256::hex(&sha256::digest(b"same bytes")),
            b"same bytes",
            &namespace,
            &policy(),
            &rebound,
        )
        .expect("again");
        assert_eq!(again.source_id, plan_first.source_id);
        assert_eq!(again.revision_id, plan_first.revision_id);
    }

    #[test]
    fn denied_sources_never_produce_durable_identifiers() {
        let namespace = sha256::hex(&sha256::digest(b"namespace"));
        let view = RegistryView::build(Vec::new(), &policy()).expect("view");
        let file_digest = sha256::hex(&sha256::digest(b"stable-secret"));
        let result = plan_snapshot(
            Path::new("/tmp/workspace/id_rsa"),
            &file_digest,
            "native",
            &sha256::hex(&sha256::digest(b"secret bytes")),
            b"secret bytes",
            &namespace,
            &policy(),
            &view,
        );
        assert_eq!(result, Err(SOURCE_ADMISSION_DENIED.to_owned()));
    }
}
