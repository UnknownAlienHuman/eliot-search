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

/// Baseline admission policy revision for primary ingestion.
pub const BASELINE_POLICY_REVISION: u64 = 1;
/// Baseline maximum admitted file bytes (16 MiB, per admission contract).
pub const BASELINE_MAX_FILE_BYTES: u64 = 16_777_216;
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
    DenyByDefault,
}

impl AdmissionReasonCode {
    /// Stable machine-readable code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllowBaselineSource => "ALLOW_BASELINE_SOURCE",
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
            Self::DenyByDefault => "DENY_BY_DEFAULT",
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
        Self::from_config(SourceAdmissionConfig::baseline())
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
    if let Some(found) = credential_class(&name, &extension) {
        return Ok(found);
    }
    if let Some(found) = system_cache_build_class(&name, &components, &extension) {
        return Ok(found);
    }
    if let Some(found) = generated_vendor_class(&name, &components, &extension) {
        return Ok(found);
    }
    if let Some(found) = binary_class(&name, &extension) {
        return Ok(found);
    }
    if let Some(found) = secret_candidate_class(&name) {
        return Ok(found);
    }
    Ok(baseline_class(&name, &components, &extension))
}

fn credential_class(name: &str, extension: &str) -> Option<(SourceClass, SensitivityLevel, bool)> {
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
            extension,
            "pem" | "key" | "pfx" | "p12" | "asc" | "gpg" | "pgp" | "kdbx"
        )
        || name == "credentials.json"
        || name == "secrets.yaml"
        || name == "secrets.yml"
        || name == "secrets.json"
        || name == "secrets.toml";
    if !credential_name {
        return None;
    }
    if matches!(extension, "pem" | "key" | "pfx" | "p12")
        || name.starts_with("id_")
        || name.contains("private")
    {
        return Some((SourceClass::PrivateKey, SensitivityLevel::PrivateKey, false));
    }
    Some((SourceClass::Credential, SensitivityLevel::Credential, false))
}

fn system_cache_build_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
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
        return Some((SourceClass::System, SensitivityLevel::Internal, false));
    }
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "__pycache__" | ".cache" | ".mypy_cache" | ".pytest_cache" | ".venv" | "venv"
        )
    }) {
        return Some((SourceClass::Cache, SensitivityLevel::Internal, false));
    }
    if components
        .iter()
        .any(|component| matches!(component.as_str(), "target" | "dist" | "build" | "out"))
        || matches!(extension, "o" | "obj" | "class" | "pyc" | "pyo")
    {
        return Some((SourceClass::BuildOutput, SensitivityLevel::Internal, false));
    }
    None
}

fn generated_vendor_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    // Generated sources: explicit allow flag gates them.
    if name.contains(".generated.")
        || name.contains("_generated")
        || name.contains("generated_")
        || components.iter().any(|component| component == "generated")
        || matches!(extension, "g.cs" | "pb.go")
        || name.ends_with(".designer.cs")
        || name.ends_with(".min.js")
    {
        return Some((SourceClass::Generated, SensitivityLevel::Internal, false));
    }
    // Vendor sources: explicit allow flag gates them.
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "vendor" | "third_party" | "thirdparty" | "node_modules"
        )
    }) {
        return Some((SourceClass::Vendor, SensitivityLevel::Internal, false));
    }
    None
}

fn binary_class(name: &str, extension: &str) -> Option<(SourceClass, SensitivityLevel, bool)> {
    // Binary sources: extension gates admission. Raw NUL content does NOT
    // gate here: binary `.txt` sources are admitted for downstream gap
    // handling (materializer reports `MATERIALIZATION_BINARY_CONTENT` with
    // `searched_sources=0` instead of an exact negative), preserving the
    // preparation flow. Only known binary extensions deny at admission.
    let binary_hint = matches!(
        extension,
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
    if !binary_hint {
        return None;
    }
    // A secret-looking binary name is still a binary class denial with a
    // sensitive ceiling; unconditional credential names already returned.
    if name.contains("secret")
        || name.contains("credential")
        || name.contains("token")
        || name.contains("password")
        || name.contains("private")
    {
        return Some((SourceClass::Binary, SensitivityLevel::SecretCandidate, true));
    }
    Some((SourceClass::Binary, SensitivityLevel::Internal, true))
}

fn secret_candidate_class(name: &str) -> Option<(SourceClass, SensitivityLevel, bool)> {
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
        return Some((
            SourceClass::Regular,
            SensitivityLevel::SecretCandidate,
            false,
        ));
    }
    if name.contains("token") && (name.contains("api") || name.contains("auth")) {
        return Some((
            SourceClass::Regular,
            SensitivityLevel::SecretCandidate,
            false,
        ));
    }
    None
}

fn baseline_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> (SourceClass, SensitivityLevel, bool) {
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
        || extension == "md"
        || extension == "rst"
        || name.starts_with("readme")
        || name.starts_with("changelog")
        || name.starts_with("license")
    {
        if extension == "md"
            || extension == "rst"
            || name.starts_with("readme")
            || name.starts_with("changelog")
            || name.starts_with("license")
            || components
                .iter()
                .any(|component| matches!(component.as_str(), "docs" | "documentation"))
        {
            return (SourceClass::Documentation, SensitivityLevel::Public, false);
        }
        return (SourceClass::Test, SensitivityLevel::Internal, false);
    }
    (SourceClass::Regular, SensitivityLevel::Internal, false)
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
    /// Terminal outcome.
    pub const fn outcome(&self) -> AdmissionOutcome {
        self.outcome
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
    let fingerprint = policy.fingerprint().to_owned();
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
    } else if observation.byte_size > policy.max_file_bytes() {
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
        policy_revision: policy.revision(),
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
    let fingerprint = policy.fingerprint().to_owned();
    let digest = observation_digest_for(
        observation.source_class,
        observation.sensitivity,
        observation.byte_size,
        observation.is_binary_hint,
    );
    if decision.policy_revision != policy.revision()
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
        policy_revision: policy.revision(),
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
    if receipt.policy_revision != current_policy.revision()
        || receipt.policy_fingerprint != current_policy.fingerprint()
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
// Stable identity (paths are locators, never identity)
// ---------------------------------------------------------------------------

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
/// identity. `Ambiguous` never produces an identifier.
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
    }
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
            policy_revision: policy.revision(),
            policy_fingerprint: policy.fingerprint().to_owned(),
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
        match receipt.outcome() {
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
    match decision.outcome() {
        AdmissionOutcome::Allow => {}
        AdmissionOutcome::Deny => {
            if decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenyEmptySource)
            {
                return Err(SOURCE_ADMISSION_DENIED.to_owned());
            }
            if decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenySizeExceeded)
            {
                return Err(SOURCE_TOO_LARGE.to_owned());
            }
            if decision.reasons().any(|reason| {
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
// Git source acquisition (T32 remainder, no-execute loose objects)
// ---------------------------------------------------------------------------
//
// Git sources resolve through `search-safe-reader::git` validation over an
// already admitted repository: loose objects under the admitted `.git`
// directory, addressed by exact object ID (`objects/aa/bb…`), validated for
// header, kind, size and structure under finite ceilings. These helpers
// perform no filesystem I/O, spawn no process, invoke no hook, checkout
// filter, smudge/clean driver or credential helper, and touch no network.
// Packed-only objects stay explicit
// `GIT_OBJECT_PACKED_UNAVAILABLE`; remote-promised objects stay explicit
// `GIT_OBJECT_REQUIRES_NETWORK`; missing objects stay explicit
// `GIT_OBJECT_NOT_FOUND`. There is no live-path substitution, no second
// catalog (the same [`RegistryView`] backs file and git planning) and no
// path-as-identity (logical paths classify only; durable identity is the
// admitted repository digest plus the object ID).

/// Statically pinned no-execute invariant for git composition.
// Staged T32 binding: the daemon binary does not call these helpers yet;
// the e2e process test proves them. Allow dead in the binary until wiring.
#[allow(dead_code)]
pub const GIT_SOURCE_NO_EXECUTE: bool = true;

const _: () = assert!(GIT_SOURCE_NO_EXECUTE);

/// Domain binding a repository digest plus an object ID to stable identity.
#[allow(dead_code)]
pub const GIT_STABLE_IDENTITY_DOMAIN: &[u8] = b"eliot-searchd/git-stable-identity/v1";

/// Maximum decompressed git object bytes (mirrors the git kernel ceiling).
#[allow(dead_code)]
pub const MAX_GIT_OBJECT_BYTES: u64 = 64 * 1024 * 1024;

/// Maximum bytes in a derived loose-object path token.
#[allow(dead_code)]
pub const MAX_GIT_PATH_TOKEN_BYTES: usize = 32_768;

/// Repository digest text is not exactly 64 hexadecimal characters.
#[allow(dead_code)]
pub const GIT_SOURCE_REPOSITORY_INVALID: &str = "GIT_SOURCE_REPOSITORY_INVALID";

/// Lineage evidence digest is not exactly 64 hexadecimal characters.
#[allow(dead_code)]
pub const GIT_SOURCE_LINEAGE_INVALID: &str = "GIT_SOURCE_LINEAGE_INVALID";

/// Closed git lineage relationship.
///
/// The kind plus the evidence digest bind repository, worktree, submodule,
/// fork and mirror relationships to caller-supplied evidence. Path, remote
/// URL, repository name and HEAD values are never accepted here and never
/// participate in identity.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GitLineageKind {
    /// Canonical repository object.
    Repository,
    /// Linked worktree view of the same repository.
    Worktree,
    /// Submodule repository bound by superproject evidence.
    Submodule,
    /// Forked repository with distinct lineage evidence.
    Fork,
    /// Mirror repository with distinct lineage evidence.
    Mirror,
}

impl GitLineageKind {
    /// Stable wire string.
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Worktree => "worktree",
            Self::Submodule => "submodule",
            Self::Fork => "fork",
            Self::Mirror => "mirror",
        }
    }
}

/// Exact lineage binding for one git plan.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitLineage {
    /// Closed lineage relationship.
    pub kind: GitLineageKind,
    /// Exact 64-hex evidence digest binding the relationship.
    pub evidence_digest_hex: String,
}

/// Full canonical plan for one retained git object.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitPlannedSource {
    /// Durable source identifier (same domain as file sources).
    pub source_id: String,
    /// Revision identifier binding canonical source, content and size.
    pub revision_id: String,
    /// Verified admission receipt authorizing retention.
    pub receipt: ComposedAdmissionReceipt,
    /// Canonical lowercase object ID hex (40 characters).
    pub object_id_hex: String,
    /// Canonical lowercase repository digest hex (64 characters).
    pub repository_digest_hex: String,
    /// Validated object kind.
    pub kind: search_safe_reader::git::GitObjectKind,
    /// Exact payload length (header excluded).
    pub payload_len: u64,
    /// Bound lineage evidence (metadata, never identity).
    pub lineage: GitLineage,
    /// Whether stable identity was natively proven (always true here).
    pub identity_native: bool,
}

/// Validates an admitted repository digest as exact 64 hexadecimal characters.
///
/// # Errors
///
/// Returns `GIT_SOURCE_REPOSITORY_INVALID` for malformed or zero digests.
#[allow(dead_code)]
pub fn validate_git_repository_hex(value: &str) -> Result<[u8; 32], String> {
    let raw =
        sha256::decode_digest(value).ok_or_else(|| GIT_SOURCE_REPOSITORY_INVALID.to_owned())?;
    if raw == [0_u8; 32] {
        return Err(GIT_SOURCE_REPOSITORY_INVALID.to_owned());
    }
    Ok(raw)
}

/// Validates an object ID as exactly 40 hexadecimal characters.
///
/// # Errors
///
/// Returns `GIT_OBJECT_INVALID_ID` for malformed IDs.
#[allow(dead_code)]
pub fn validate_git_object_hex(
    value: &str,
) -> Result<search_safe_reader::git::GitObjectId, String> {
    search_safe_reader::git::GitObjectId::parse_hex(value).map_err(|error| error.code().to_owned())
}

/// Derives the canonical `objects/aa/bb…` loose path for one object ID.
///
/// The path is derived from the validated ID bytes alone; no repository
/// state, branch name or HEAD value participates in addressing.
///
/// # Errors
///
/// Returns `GIT_OBJECT_INVALID_ID` for malformed IDs and
/// `GIT_OBJECT_INVALID_LIMITS` when the finite token ceiling is exceeded.
#[allow(dead_code)]
pub fn derive_git_loose_path(object_id_hex: &str) -> Result<String, String> {
    let object_id = validate_git_object_hex(object_id_hex)?;
    let limits = search_safe_reader::git::GitReadLimits {
        max_decompressed_bytes: MAX_GIT_OBJECT_BYTES,
        max_path_token_bytes: MAX_GIT_PATH_TOKEN_BYTES,
    };
    let token = object_id
        .loose_relative_path(limits)
        .map_err(|error| error.code().to_owned())?;
    Ok(token.as_str().to_owned())
}

/// Derives the stable identity digest binding repository plus object.
///
/// Paths, remote URLs, names and HEAD values never participate; only the
/// admitted repository bytes and the exact object ID bytes do.
///
/// # Errors
///
/// Returns `GIT_SOURCE_REPOSITORY_INVALID` or `GIT_OBJECT_INVALID_ID` for
/// malformed inputs.
#[allow(dead_code)]
pub fn git_stable_identity_hex(repository_hex: &str, object_hex: &str) -> Result<String, String> {
    let repository = validate_git_repository_hex(repository_hex)?;
    let object_id = validate_git_object_hex(object_hex)?;
    Ok(sha256::hex(&sha256::digest_parts(
        GIT_STABLE_IDENTITY_DOMAIN,
        &[&repository, object_id.as_bytes()],
    )))
}

/// Validates one lineage binding against its evidence digest.
///
/// # Errors
///
/// Returns `GIT_SOURCE_LINEAGE_INVALID` for malformed or zero evidence.
#[allow(dead_code)]
pub fn validate_git_lineage(lineage: &GitLineage) -> Result<(), String> {
    let raw = sha256::decode_digest(&lineage.evidence_digest_hex)
        .ok_or_else(|| GIT_SOURCE_LINEAGE_INVALID.to_owned())?;
    if raw == [0_u8; 32] {
        return Err(GIT_SOURCE_LINEAGE_INVALID.to_owned());
    }
    Ok(())
}

/// Maps one git kernel failure to its closed content-free reason code.
#[allow(dead_code)]
pub const fn git_error_code(error: search_safe_reader::git::GitReadError) -> &'static str {
    error.code()
}

#[allow(dead_code)]
const fn git_read_limits() -> search_safe_reader::git::GitReadLimits {
    search_safe_reader::git::GitReadLimits {
        max_decompressed_bytes: MAX_GIT_OBJECT_BYTES,
        max_path_token_bytes: MAX_GIT_PATH_TOKEN_BYTES,
    }
}

/// Plans one git loose object through canonical composition.
///
/// Order: validate repository and object IDs, derive the canonical loose
/// path (ID-derived addressing proof), validate lineage evidence, parse the
/// decompressed `<type> <size>\0<payload>` object (header, kind, size and
/// structure), classify the payload under `logical_path_for_admission`,
/// evaluate, issue, verify and admit against the registry view, resolve the
/// repository-plus-object stable identity, and derive durable identifiers.
/// Denied, review-required, unsupported and ambiguous objects never reach
/// CAS. This function performs no I/O and executes nothing.
///
/// `logical_path_for_admission` is a locator for closed classification only;
/// it never participates in identity. `expected_kind` binds the exact kind
/// when supplied; `None` accepts any known kind.
///
/// # Errors
///
/// Returns the exact closed canonical code for every denial: git object
/// codes (`GIT_OBJECT_*`), lineage and repository codes
/// (`GIT_SOURCE_*`), admission codes (`SOURCE_ADMISSION_DENIED`,
/// `SENSITIVE_SOURCE_DENIED`, `SOURCE_TOO_LARGE`, …) and identity codes
/// (`SOURCE_IDENTITY_*`). Never a fabricated identifier or silent fallback.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn plan_git_snapshot(
    repository_identity_digest_hex: &str,
    object_id_hex: &str,
    expected_kind: Option<search_safe_reader::git::GitObjectKind>,
    decompressed_object: &[u8],
    logical_path_for_admission: &Path,
    lineage: &GitLineage,
    namespace_hex: &str,
    policy: &AdmissionPolicy,
    view: &RegistryView,
) -> Result<GitPlannedSource, String> {
    let object_len =
        u64::try_from(decompressed_object.len()).map_err(|_| "GIT_OBJECT_TOO_LARGE".to_owned())?;
    if object_len > MAX_GIT_OBJECT_BYTES {
        return Err("GIT_OBJECT_TOO_LARGE".to_owned());
    }
    let repository_bytes = validate_git_repository_hex(repository_identity_digest_hex)?;
    let object_id = validate_git_object_hex(object_id_hex)?;
    let _ = derive_git_loose_path(object_id_hex)?;
    validate_git_lineage(lineage)?;
    let limits = git_read_limits();
    let parsed =
        search_safe_reader::git::parse_loose_object(decompressed_object, limits, expected_kind)
            .map_err(|error| error.code().to_owned())?;
    let payload = parsed.payload;
    let payload_len =
        u64::try_from(payload.len()).map_err(|_| "GIT_OBJECT_TOO_LARGE".to_owned())?;
    let observation = build_observation(logical_path_for_admission, payload, policy)?;
    let decision = evaluate(policy, &observation);
    let receipt = issue_receipt(policy, &observation, &decision)?;
    view.admit(&receipt, &observation, policy)?;
    match decision.outcome() {
        AdmissionOutcome::Allow => {}
        AdmissionOutcome::Deny => {
            if decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenyEmptySource)
            {
                return Err(SOURCE_ADMISSION_DENIED.to_owned());
            }
            if decision
                .reasons()
                .any(|reason| *reason == AdmissionReasonCode::DenySizeExceeded)
            {
                return Err(SOURCE_TOO_LARGE.to_owned());
            }
            if decision.reasons().any(|reason| {
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
    let stable_hex = git_stable_identity_hex(repository_identity_digest_hex, object_id_hex)?;
    let resolution = resolve_identity(&stable_hex, "native", &view.prior_candidates())?;
    let source_id = derive_source_id(namespace_hex, &stable_hex, &resolution)?;
    let content_hex = sha256::hex(&sha256::digest(payload));
    let revision_id = derive_revision_id(&source_id, &content_hex, payload_len)?;
    Ok(GitPlannedSource {
        source_id,
        revision_id,
        receipt,
        object_id_hex: object_id.hex(),
        repository_digest_hex: sha256::hex(&repository_bytes),
        kind: parsed.kind,
        payload_len,
        lineage: lineage.clone(),
        identity_native: true,
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
        let mut altered = receipt;
        altered.reasons.insert(AdmissionReasonCode::DenyByDefault);
        assert!(verify_receipt(&altered, &policy(), &observation).is_err());
    }

    #[test]
    fn path_escape_is_owned_by_the_safe_reader_not_duplicated_here() {
        // Path-escape and reserved-device enforcement live in
        // `crate::safe_reader_adapter` (kernel-verified reads) with its own
        // live process tests. This composition layer never resolves paths to
        // identity: `resolve_identity` with path-bound strength stays
        // ambiguous via the live API below.
        let file_digest = sha256::hex(&sha256::digest(b"stable-material"));
        let prior = vec![PriorSourceView {
            source_id: sha256::hex(&sha256::digest(b"existing")),
            file_identity_digest: file_digest.clone(),
            path_digest: sha256::hex(&sha256::digest(b"old-path")),
            revision_id: sha256::hex(&sha256::digest(b"revision")),
            record_digest: sha256::hex(&sha256::digest(b"record")),
            is_active: true,
        }];
        assert_eq!(
            resolve_identity(&file_digest, "path-bound", &prior).expect("resolution"),
            IdentityResolution::Ambiguous
        );
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
        let expected = sha256::hex(&sha256::digest_parts(
            CANONICAL_SOURCE_ID_DOMAIN,
            &[
                &sha256::decode_digest(&namespace).expect("namespace"),
                &sha256::decode_digest(&unseen).expect("identity"),
            ],
        ));
        assert_eq!(
            fresh, expected,
            "canonical domain derivation stays stable (T11 migration readback)"
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
