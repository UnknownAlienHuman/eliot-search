//! Checkpointed retention and owner-governed reconstitution of one complete v2
//! admission policy.
//!
//! Capture reads public getters only. Restore rebuilds the caller section input
//! and the raw policy input, then runs the real section validator and the real
//! policy compiler, and finally re-encodes the restored model for an exact
//! comparison. A supplied fingerprint never constructs a model here.

use std::sync::Arc;

use crate::codec::MAX_RECORD_FIELDS;
use crate::codec::{self, CanonicalRecord, CodecError};
use search_contracts::canonical::CanonicalValue;
use search_contracts::{BoundedMap, CanonicalKey};
use search_source_admission::policy_v2::{
    CanonicalRuleV2, CheckpointFailureV2, CompileLimitsV2, CompiledAdmissionPolicyV2,
    POLICY_PROFILE, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2, RuleInputV2,
    compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, AdmissionError, ConfigSectionInput, LocatorClass, RuleEffect,
    SectionPlatform, SensitivityLevel, SourceClass, validate_section,
};

/// Inclusive archive ceiling for one retained whole policy record.
///
/// This is an in-memory parser bound, not a durable journal limit or a measured
/// maximum accepted archive. The storage owner must check its actual complete
/// record/operation envelopes without truncating or silently narrowing policies.
pub const MAX_RETAINED_POLICY_BYTES: usize = 256 * 1024;
/// Closed retention schema, separate from the admission fingerprint preimage.
pub const RETAINED_POLICY_PROFILE_V2: &str = "eliot.source-original-policy@2";

/// Caller-owned deadline and cancellation checkpoint translation.
fn checkpoint(
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<(), AdmissionError> {
    probe().map_err(|error| match error {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    })
}

impl From<CodecError> for AdmissionError {
    fn from(error: CodecError) -> Self {
        match error {
            CodecError::NonCanonical => Self::PolicyUnknownField,
        }
    }
}

/// Complete retained v2 policy provenance, shared cheaply by clone.
#[derive(Clone)]
pub struct RetainedPolicyV2 {
    compiled: Arc<CompiledAdmissionPolicyV2>,
    encoded: Arc<[u8]>,
}

impl core::fmt::Debug for RetainedPolicyV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RetainedPolicyV2")
            .field("encoded_bytes", &self.encoded.len())
            .finish_non_exhaustive()
    }
}

impl RetainedPolicyV2 {
    /// Captures one already compiled policy and its canonical encoded inputs.
    ///
    /// Every value comes from a public getter on the compiled model. The
    /// compiled model is rebuilt through the real section validator and policy
    /// compiler rather than cloned, so the retained archive and the retained
    /// model always agree. No fingerprint is accepted or stored as authority.
    ///
    /// # Errors
    /// Returns the original section/compiler refusal, a canonical archive error
    /// or caller cancellation/deadline failure without a retained policy.
    pub fn capture(
        policy: &CompiledAdmissionPolicyV2,
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, AdmissionError> {
        checkpoint(probe)?;
        let encoded = encode_policy(policy, probe)?;
        checkpoint(probe)?;
        Self::restore(&encoded, platform, capabilities, probe)
    }

    /// Exact canonical encoded inputs retained for this policy.
    #[must_use]
    pub fn encoded_inputs(&self) -> &[u8] {
        &self.encoded
    }

    /// Owner-computed policy identity of the retained compiled model.
    #[must_use]
    pub fn fingerprint(&self) -> search_source_admission::policy_v2::PolicyFingerprintV2 {
        self.compiled.fingerprint()
    }

    /// Shared compiled model retained inside this archive.
    #[must_use]
    pub fn compiled(&self) -> &CompiledAdmissionPolicyV2 {
        &self.compiled
    }
}

impl RetainedPolicyV2 {
    /// Restores one retained policy from bounded untrusted canonical bytes.
    ///
    /// The caller supplies the actual host section platform and its accepted
    /// capability profiles; this package never invents a host. The section is
    /// rebuilt through the real section validator, the policy through the real
    /// policy compiler, and the restored model is re-encoded for an exact
    /// comparison against the retained bytes. Any disagreement, unknown field,
    /// duplicate, noncanonical byte or oversize archive returns no model.
    ///
    /// # Errors
    /// Returns the original section/compiler refusal, an invalid archive error
    /// or caller cancellation/deadline failure without a retained policy.
    pub fn restore(
        bytes: &[u8],
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, AdmissionError> {
        checkpoint(probe)?;
        if bytes.is_empty() || bytes.len() > MAX_RETAINED_POLICY_BYTES {
            return Err(AdmissionError::PolicyConflict);
        }
        let restored = Self::rebuild_compiled(bytes, platform, capabilities, probe)?;
        checkpoint(probe)?;
        // The restored model must re-encode to the exact retained bytes.
        let reencoded = encode_policy(&restored, probe)?;
        if reencoded != bytes {
            return Err(AdmissionError::PolicyConflict);
        }
        checkpoint(probe)?;
        let result = Self {
            compiled: Arc::new(restored),
            encoded: Arc::from(bytes),
        };
        checkpoint(probe)?;
        Ok(result)
    }

    /// Rebuilds one compiled model from retained canonical bytes.
    fn rebuild_compiled(
        bytes: &[u8],
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<CompiledAdmissionPolicyV2, AdmissionError> {
        let record = CanonicalRecord::decode(bytes)?;
        checkpoint(probe)?;
        if record.len() != 7 || read_text(&record, "archive_profile")? != RETAINED_POLICY_PROFILE_V2
        {
            return Err(AdmissionError::PolicyUnknownField);
        }
        let schema = read_unsigned(&record, "schema")?;
        if schema != u64::from(POLICY_SCHEMA_VERSION) {
            return Err(AdmissionError::PolicySchemaUnsupported);
        }
        let profile = read_text(&record, "profile")?;
        if profile != POLICY_PROFILE {
            return Err(AdmissionError::PolicySchemaUnsupported);
        }
        let revision = read_unsigned(&record, "revision")?;
        let limits = read_limits(&record)?;
        let section = read_section(&record)?;
        checkpoint(probe)?;
        let rules = read_rules(&record, limits, probe)?;
        checkpoint(probe)?;

        let validated = validate_section(&section, platform, capabilities)?;
        checkpoint(probe)?;
        let compiled = compile_policy_v2(
            &PolicyInputV2 {
                schema_version: POLICY_SCHEMA_VERSION,
                revision,
                section: validated,
                rules,
                unknown_fields: Vec::new(),
            },
            limits,
            probe,
        )?;
        checkpoint(probe)?;
        Ok(compiled)
    }
}

/// Encodes one compiled policy into its canonical retained record.
fn encode_policy(
    policy: &CompiledAdmissionPolicyV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<Vec<u8>, AdmissionError> {
    checkpoint(probe)?;
    let mut record = CanonicalRecord::new();
    record.insert("archive_profile", codec::text(RETAINED_POLICY_PROFILE_V2)?)?;
    record.insert("schema", codec::unsigned(u64::from(POLICY_SCHEMA_VERSION)))?;
    record.insert("profile", codec::text(POLICY_PROFILE)?)?;
    record.insert("revision", codec::unsigned(policy.revision().get()))?;
    record.insert("section", encode_section(policy.section())?)?;
    record.insert("limits", encode_limits(policy.limits())?)?;
    record.insert("rules", encode_rules(policy.rules(), probe)?)?;
    let encoded = record.encode()?;
    if encoded.len() > MAX_RETAINED_POLICY_BYTES {
        return Err(AdmissionError::PolicyConflict);
    }
    checkpoint(probe)?;
    Ok(encoded)
}

/// Encodes the five validated section fields.
fn encode_section(
    section: &search_source_admission::ValidatedAdmissionConfig,
) -> Result<CanonicalValue, AdmissionError> {
    let mut record = CanonicalRecord::new();
    record.insert(
        "bootstrap_policy_profile",
        codec::text(section.bootstrap_policy_profile())?,
    )?;
    record.insert("max_file_bytes", codec::unsigned(section.max_file_bytes()))?;
    record.insert("allow_generated", codec::boolean(section.allow_generated()))?;
    record.insert("allow_vendor", codec::boolean(section.allow_vendor()))?;
    record.insert("allow_binary", codec::boolean(section.allow_binary()))?;
    Ok(CanonicalValue::Object(record.into_inner()))
}

/// Encodes all four compile ceilings.
fn encode_limits(limits: CompileLimitsV2) -> Result<CanonicalValue, AdmissionError> {
    let mut record = CanonicalRecord::new();
    record.insert("max_rules", codec::unsigned(limits.max_rules as u64))?;
    record.insert(
        "max_pattern_bytes",
        codec::unsigned(limits.max_pattern_bytes as u64),
    )?;
    record.insert(
        "max_total_pattern_bytes",
        codec::unsigned(limits.max_total_pattern_bytes as u64),
    )?;
    record.insert(
        "max_encoded_bytes",
        codec::unsigned(limits.max_encoded_bytes as u64),
    )?;
    Ok(CanonicalValue::Object(record.into_inner()))
}

/// Encodes every canonical rule in its retained order.
fn encode_rules(
    rules: &[CanonicalRuleV2],
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<CanonicalValue, AdmissionError> {
    let mut values = Vec::with_capacity(rules.len());
    for rule in rules {
        checkpoint(probe)?;
        values.push(encode_rule(rule)?);
    }
    Ok(codec::array(values)?)
}

/// Encodes one canonical rule with its exact predicate operand.
fn encode_rule(rule: &CanonicalRuleV2) -> Result<CanonicalValue, AdmissionError> {
    let mut record = CanonicalRecord::new();
    record.insert("id", codec::text(rule.id())?)?;
    record.insert("precedence", codec::unsigned(u64::from(rule.precedence())))?;
    record.insert("source_class", codec::text(rule.source_class().as_str())?)?;
    record.insert("predicate", encode_predicate(rule.predicate())?)?;
    record.insert("effect", codec::text(effect_tag(rule.effect()))?)?;
    Ok(CanonicalValue::Object(record.into_inner()))
}

/// Encodes one predicate operator and its exact operand.
fn encode_predicate(predicate: &PredicateV2) -> Result<CanonicalValue, AdmissionError> {
    let mut record = CanonicalRecord::new();
    record.insert("operator", codec::text(predicate.as_str())?)?;
    match predicate {
        PredicateV2::Always | PredicateV2::SourceClassIs => {}
        PredicateV2::SensitivityAtLeast(level) => {
            record.insert("level", codec::text(level.as_str())?)?;
        }
        PredicateV2::SizeGreaterThan(value) => {
            record.insert("bytes", codec::unsigned(*value))?;
        }
        PredicateV2::LocatorClassIs(class) => {
            record.insert("class", codec::text(class.as_str())?)?;
        }
        PredicateV2::RelativePathGlobMatches(pattern) => {
            record.insert("pattern", codec::text(pattern)?)?;
        }
    }
    Ok(CanonicalValue::Object(record.into_inner()))
}

/// Stable closed effect tag.
const fn effect_tag(effect: RuleEffect) -> &'static str {
    match effect {
        RuleEffect::Allow => "allow",
        RuleEffect::Deny => "deny",
        RuleEffect::Review => "review",
        RuleEffect::Unsupported => "unsupported",
    }
}

/// Reads one required unsigned integer field.
fn read_unsigned(record: &CanonicalRecord, key: &str) -> Result<u64, AdmissionError> {
    match record.get(key) {
        Some(CanonicalValue::U64(value)) => Ok(*value),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Reads one required text field.
fn read_text<'a>(record: &'a CanonicalRecord, key: &str) -> Result<&'a str, AdmissionError> {
    match record.get(key) {
        Some(CanonicalValue::Text(value)) => Ok(value.as_str()),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Reads one required nested record field.
fn read_record<'a>(
    record: &'a CanonicalRecord,
    key: &str,
) -> Result<&'a BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>, AdmissionError> {
    match record.get(key) {
        Some(CanonicalValue::Object(fields)) => Ok(fields),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Reads all four compile ceilings.
fn read_limits(record: &CanonicalRecord) -> Result<CompileLimitsV2, AdmissionError> {
    let fields = read_record(record, "limits")?;
    let limits = CompileLimitsV2 {
        max_rules: usize::try_from(field_unsigned(fields, "max_rules")?)
            .map_err(|_| AdmissionError::PolicyConflict)?,
        max_pattern_bytes: usize::try_from(field_unsigned(fields, "max_pattern_bytes")?)
            .map_err(|_| AdmissionError::PolicyConflict)?,
        max_total_pattern_bytes: usize::try_from(field_unsigned(
            fields,
            "max_total_pattern_bytes",
        )?)
        .map_err(|_| AdmissionError::PolicyConflict)?,
        max_encoded_bytes: usize::try_from(field_unsigned(fields, "max_encoded_bytes")?)
            .map_err(|_| AdmissionError::PolicyConflict)?,
    };
    for (value, maximum) in [
        (
            limits.max_rules,
            search_source_admission::policy_v2::MAX_RULES,
        ),
        (
            limits.max_pattern_bytes,
            search_source_admission::policy_v2::MAX_PATTERN_BYTES,
        ),
        (
            limits.max_total_pattern_bytes,
            search_source_admission::policy_v2::MAX_TOTAL_PATTERN_BYTES,
        ),
        (
            limits.max_encoded_bytes,
            search_source_admission::policy_v2::MAX_POLICY_PREIMAGE_BYTES,
        ),
    ] {
        if value == 0 || value > maximum {
            return Err(AdmissionError::PolicyConflict);
        }
    }
    Ok(limits)
}

/// Reads one unsigned integer field from a nested record.
fn field_unsigned(
    fields: &BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>,
    key: &str,
) -> Result<u64, AdmissionError> {
    let key = CanonicalKey::new(key).map_err(CodecError::from)?;
    match fields.get(&key) {
        Some(CanonicalValue::U64(value)) => Ok(*value),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Reads one text field from a nested record.
fn field_text<'a>(
    fields: &'a BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>,
    key: &str,
) -> Result<&'a str, AdmissionError> {
    let key = CanonicalKey::new(key).map_err(CodecError::from)?;
    match fields.get(&key) {
        Some(CanonicalValue::Text(value)) => Ok(value.as_str()),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Reads one boolean field from a nested record.
fn field_boolean(
    fields: &BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>,
    key: &str,
) -> Result<bool, AdmissionError> {
    let key = CanonicalKey::new(key).map_err(CodecError::from)?;
    match fields.get(&key) {
        Some(CanonicalValue::Bool(value)) => Ok(*value),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Rebuilds the caller section input from the retained five fields.
fn read_section(record: &CanonicalRecord) -> Result<ConfigSectionInput, AdmissionError> {
    let fields = read_record(record, "section")?;
    Ok(ConfigSectionInput {
        bootstrap_policy_profile: field_text(fields, "bootstrap_policy_profile")?.to_owned(),
        max_file_bytes: field_unsigned(fields, "max_file_bytes")?,
        allow_generated: field_boolean(fields, "allow_generated")?,
        allow_vendor: field_boolean(fields, "allow_vendor")?,
        allow_binary: field_boolean(fields, "allow_binary")?,
        unknown_keys: Vec::new(),
    })
}

/// Rebuilds every raw rule input, rejecting duplicates and unknown fields.
fn read_rules(
    record: &CanonicalRecord,
    limits: CompileLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<Vec<RuleInputV2>, AdmissionError> {
    let Some(CanonicalValue::Array(values)) = record.get("rules") else {
        return Err(AdmissionError::PolicyUnknownField);
    };
    if values.len() > limits.max_rules {
        return Err(AdmissionError::PolicyConflict);
    }
    let mut pattern_bytes = 0_usize;
    let mut rules = Vec::with_capacity(values.len());
    for value in values {
        checkpoint(probe)?;
        let rule = read_rule(value)?;
        if let PredicateV2::RelativePathGlobMatches(pattern) = &rule.predicate {
            pattern_bytes = pattern_bytes
                .checked_add(pattern.len())
                .ok_or(AdmissionError::PolicyConflict)?;
            if pattern.len() > limits.max_pattern_bytes
                || pattern_bytes > limits.max_total_pattern_bytes
            {
                return Err(AdmissionError::PolicyConflict);
            }
        }
        rules.push(rule);
    }
    Ok(rules)
}

/// Rebuilds one raw rule input with its exact predicate operand.
fn read_rule(value: &CanonicalValue) -> Result<RuleInputV2, AdmissionError> {
    let CanonicalValue::Object(fields) = value else {
        return Err(AdmissionError::PolicyUnknownField);
    };
    let Some(CanonicalValue::Object(predicate_fields)) =
        fields.get(&CanonicalKey::new("predicate").map_err(CodecError::from)?)
    else {
        return Err(AdmissionError::PolicyUnknownField);
    };
    if field_text(fields, "id")?.len() > search_source_admission::MAX_ID_BYTES {
        return Err(AdmissionError::PolicyConflict);
    }
    Ok(RuleInputV2 {
        id: field_text(fields, "id")?.to_owned(),
        precedence: u32::try_from(field_unsigned(fields, "precedence")?)
            .map_err(|_| AdmissionError::PolicyConflict)?,
        source_class: source_class(field_text(fields, "source_class")?)?,
        predicate: read_predicate(predicate_fields)?,
        effect: effect(field_text(fields, "effect")?)?,
        unknown_fields: Vec::new(),
    })
}

/// Rebuilds one predicate operator and its exact operand.
fn read_predicate(
    fields: &BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>,
) -> Result<PredicateV2, AdmissionError> {
    match field_text(fields, "operator")? {
        "always" => Ok(PredicateV2::Always),
        "source-class-is" => Ok(PredicateV2::SourceClassIs),
        "sensitivity-at-least" => Ok(PredicateV2::SensitivityAtLeast(sensitivity(field_text(
            fields, "level",
        )?)?)),
        "size-greater-than" => Ok(PredicateV2::SizeGreaterThan(field_unsigned(
            fields, "bytes",
        )?)),
        "locator-class-is" => Ok(PredicateV2::LocatorClassIs(locator_class(field_text(
            fields, "class",
        )?)?)),
        "relative-path-glob-matches" => {
            let pattern = field_text(fields, "pattern")?;
            if pattern.len() > search_source_admission::policy_v2::MAX_PATTERN_BYTES {
                return Err(AdmissionError::PolicyConflict);
            }
            Ok(PredicateV2::RelativePathGlobMatches(pattern.to_owned()))
        }
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Parses one closed source class tag.
fn source_class(value: &str) -> Result<SourceClass, AdmissionError> {
    match value {
        "regular" => Ok(SourceClass::Regular),
        "test" => Ok(SourceClass::Test),
        "documentation" => Ok(SourceClass::Documentation),
        "generated" => Ok(SourceClass::Generated),
        "vendor" => Ok(SourceClass::Vendor),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Parses one closed sensitivity tag.
fn sensitivity(value: &str) -> Result<SensitivityLevel, AdmissionError> {
    match value {
        "public" => Ok(SensitivityLevel::Public),
        "internal" => Ok(SensitivityLevel::Internal),
        "confidential" => Ok(SensitivityLevel::Confidential),
        "secret-candidate" => Ok(SensitivityLevel::SecretCandidate),
        "credential" => Ok(SensitivityLevel::Credential),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Parses one closed locator class tag.
fn locator_class(value: &str) -> Result<LocatorClass, AdmissionError> {
    match value {
        "normalized-file" => Ok(LocatorClass::NormalizedFile),
        "normalized-directory" => Ok(LocatorClass::NormalizedDirectory),
        "content-address" => Ok(LocatorClass::ContentAddress),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}

/// Parses one closed rule effect tag.
fn effect(value: &str) -> Result<RuleEffect, AdmissionError> {
    match value {
        "allow" => Ok(RuleEffect::Allow),
        "deny" => Ok(RuleEffect::Deny),
        "review" => Ok(RuleEffect::Review),
        "unsupported" => Ok(RuleEffect::Unsupported),
        _ => Err(AdmissionError::PolicyUnknownField),
    }
}
