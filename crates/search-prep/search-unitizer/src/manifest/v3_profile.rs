//! Explicit v3 text/code profile; legacy DIRECT profiles are not accepted here.

use search_contracts::{
    Blake3Digest32, CanonicalValue, ClosedCanonicalObject, MAX_CANONICAL_BYTES, ProfileId, UnitKind,
};

use super::digest::{bytes, hash_cbor, object, text};
use crate::{UnitizationError, UnitizationLimits};

const PROFILE_DOMAIN: &str = "eliot/cbor/unitizer-profile/v3";
const MAX_PROFILE_BYTES: usize = MAX_CANONICAL_BYTES - 128;
const MAX_PROFILE_UNITS: usize = 4096;
const MAX_PROFILE_STEPS: u64 = 32_000_000;

/// Closed representation families supported by the exact UTF-8 baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3RepresentationKind {
    /// Exact UTF-8 text without structural or predicate attachments.
    Text,
    /// Exact UTF-8 code bytes without inferred compiler or parser semantics.
    Code,
}

impl V3RepresentationKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Code => "code",
        }
    }
}

/// Overlap behavior for the v3 baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3OverlapPolicy {
    /// Adjacent units share no bytes; split-line continuation stays contiguous.
    None,
}

/// Required native-coordinate policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3AnchorPolicy {
    /// Exact source-backed text-byte anchors; transformed maps are refused.
    ExactTextBytes,
}

/// Structural and configuration-predicate attachment policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3AttachmentPolicy {
    /// Both attachments must be absent, rather than silently discarded.
    Absent,
}

/// Omission and gap policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3OmissionPolicy {
    /// Every representation byte must be accounted for exactly once.
    Forbidden,
}

/// Behavior for the exact empty representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V3EmptyPolicy {
    /// Empty native representations are rejected by the existing materializer.
    Reject,
}

/// Untrusted, fully explicit v3 baseline profile descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V3UnitizerProfileDescriptor {
    /// Bounded contract profile name, distinct from the computed profile ID.
    pub profile_name: ProfileId,
    /// Nonzero monotone profile revision.
    pub profile_revision: u64,
    /// Text or code bytes accepted by this baseline.
    pub representation_kind: V3RepresentationKind,
    /// Only File, Section and Doc are supported.
    pub unit_kind: UnitKind,
    /// Revision of the reused exact UTF-8 layout algorithm; exactly one.
    pub boundary_revision: u64,
    /// No byte overlap between consecutive units.
    pub overlap_policy: V3OverlapPolicy,
    /// Exact text-byte native coordinate basis.
    pub anchor_policy: V3AnchorPolicy,
    /// No structural identity or configuration predicate.
    pub attachment_policy: V3AttachmentPolicy,
    /// No intentionally omitted or uncovered bytes.
    pub omission_policy: V3OmissionPolicy,
    /// Empty input cannot produce a v3 manifest under this baseline.
    pub empty_policy: V3EmptyPolicy,
    /// Explicit finite input, boundary, line and occurrence budgets.
    pub limits: UnitizationLimits,
    /// Minimum nonempty unit size; exactly one byte.
    pub min_unit_bytes: usize,
    /// Positive scalar-count ceiling for one unit, at most its byte ceiling.
    pub max_unit_scalars: usize,
    /// Positive logical-line ceiling for one unit.
    pub max_unit_lines: usize,
    /// Maximum native-anchor depth; exactly one for text-byte anchors.
    pub max_anchor_depth: u8,
    /// Positive canonical manifest byte ceiling, excluding its digest domain.
    pub max_manifest_bytes: usize,
    /// Positive deterministic work ceiling, at most 32,000,000 steps.
    pub max_steps: u64,
    /// Nonzero digest binding the exact qualification fixture set.
    pub fixture_digest: Blake3Digest32,
}

/// Validated descriptor and shared-owner-computed v3 profile identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedV3UnitizerProfile {
    descriptor: V3UnitizerProfileDescriptor,
    id: ProfileId,
}

impl ValidatedV3UnitizerProfile {
    /// Exact immutable descriptor whose full canonical value determines the ID.
    #[must_use]
    pub const fn descriptor(&self) -> &V3UnitizerProfileDescriptor {
        &self.descriptor
    }

    /// Contract profile ID: unitizer-v3- followed by the full BLAKE3 digest.
    #[must_use]
    pub const fn id(&self) -> &ProfileId {
        &self.id
    }

    /// Explicit finite layout budgets.
    #[must_use]
    pub const fn limits(&self) -> UnitizationLimits {
        self.descriptor.limits
    }

    /// Nonzero revision bound into the profile identity.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.descriptor.profile_revision
    }
}

/// Validates the descriptor and computes its ID through the shared digest owner.
///
/// Unsupported kinds, revisions, policies, empty fixture digests or inconsistent
/// budgets fail closed. This validates a fixture binding, not fixture execution.
pub fn validate_v3_unitizer_profile(
    descriptor: &V3UnitizerProfileDescriptor,
) -> Result<ValidatedV3UnitizerProfile, UnitizationError> {
    let value = profile_value(descriptor)?;
    let digest = hash_cbor(PROFILE_DOMAIN, &value, MAX_CANONICAL_BYTES)?;
    let id = ProfileId::new(format!("unitizer-v3-{digest}"))
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)?;
    Ok(ValidatedV3UnitizerProfile {
        descriptor: descriptor.clone(),
        id,
    })
}

fn validate_descriptor(d: &V3UnitizerProfileDescriptor) -> Result<(), UnitizationError> {
    let limits = d
        .limits
        .validate()
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)?;
    if d.profile_revision == 0
        || d.boundary_revision != 1
        || !matches!(
            d.unit_kind,
            UnitKind::File | UnitKind::Section | UnitKind::Doc
        )
        || limits.max_input_bytes > MAX_PROFILE_BYTES
        || limits.max_unit_bytes > limits.max_input_bytes
        || limits.max_lines > MAX_PROFILE_BYTES
        || limits.max_units > MAX_PROFILE_UNITS
        || d.min_unit_bytes != 1
        || d.max_unit_scalars == 0
        || d.max_unit_scalars > limits.max_unit_bytes
        || d.max_unit_lines == 0
        || d.max_unit_lines > limits.max_lines
        || d.max_unit_lines > limits.max_unit_bytes
        || d.max_anchor_depth != 1
        || d.max_manifest_bytes == 0
        || d.max_manifest_bytes > MAX_PROFILE_BYTES
        || d.max_steps == 0
        || d.max_steps > MAX_PROFILE_STEPS
        || d.fixture_digest.as_bytes().iter().all(|byte| *byte == 0)
    {
        return Err(UnitizationError::UnitizerProfileInvalid);
    }
    Ok(())
}

/// Produces the closed canonical descriptor; no local byte encoder is used.
pub(super) fn profile_value(
    d: &V3UnitizerProfileDescriptor,
) -> Result<CanonicalValue, UnitizationError> {
    validate_descriptor(d)?;
    let limits = object(vec![
        ("max_input_bytes", unsigned(d.limits.max_input_bytes)?),
        (
            "preferred_unit_bytes",
            unsigned(d.limits.preferred_unit_bytes)?,
        ),
        ("max_unit_bytes", unsigned(d.limits.max_unit_bytes)?),
        ("max_lines", unsigned(d.limits.max_lines)?),
        ("max_units", unsigned(d.limits.max_units)?),
    ])?;
    object(vec![
        ("profile_name", text(d.profile_name.as_str())?),
        ("profile_revision", CanonicalValue::U64(d.profile_revision)),
        ("representation_kind", text(d.representation_kind.as_str())?),
        ("unit_kind", text(d.unit_kind.as_str())?),
        (
            "boundary_revision",
            CanonicalValue::U64(d.boundary_revision),
        ),
        (
            "overlap_policy",
            text(match d.overlap_policy {
                V3OverlapPolicy::None => "none",
            })?,
        ),
        (
            "anchor_policy",
            text(match d.anchor_policy {
                V3AnchorPolicy::ExactTextBytes => "exact-text-bytes",
            })?,
        ),
        (
            "attachment_policy",
            text(match d.attachment_policy {
                V3AttachmentPolicy::Absent => "absent",
            })?,
        ),
        (
            "omission_policy",
            text(match d.omission_policy {
                V3OmissionPolicy::Forbidden => "forbidden",
            })?,
        ),
        (
            "empty_policy",
            text(match d.empty_policy {
                V3EmptyPolicy::Reject => "reject-empty",
            })?,
        ),
        ("limits", limits),
        ("min_unit_bytes", unsigned(d.min_unit_bytes)?),
        ("max_unit_scalars", unsigned(d.max_unit_scalars)?),
        ("max_unit_lines", unsigned(d.max_unit_lines)?),
        (
            "max_anchor_depth",
            CanonicalValue::U64(u64::from(d.max_anchor_depth)),
        ),
        ("max_manifest_bytes", unsigned(d.max_manifest_bytes)?),
        ("max_steps", CanonicalValue::U64(d.max_steps)),
        ("fixture_digest", bytes(d.fixture_digest.as_bytes())?),
    ])
}

/// Decodes a closed typed descriptor and recomputes its validated profile ID.
///
/// Unknown, missing, mistyped or unsupported policy fields are rejected,
/// including old DIRECT descriptors and future attachment-bearing profiles.
pub(super) fn decode_profile(
    value: CanonicalValue,
) -> Result<ValidatedV3UnitizerProfile, UnitizationError> {
    let mut fields = closed(value)?;
    let profile_name = ProfileId::new(read_text(&mut fields, "profile_name")?)
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)?;
    let profile_revision = number(&mut fields, "profile_revision")?;
    let representation_kind = match read_text(&mut fields, "representation_kind")?.as_str() {
        "text" => V3RepresentationKind::Text,
        "code" => V3RepresentationKind::Code,
        _ => return Err(UnitizationError::UnitizerProfileInvalid),
    };
    let unit_kind = UnitKind::parse(&read_text(&mut fields, "unit_kind")?)
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)?;
    let boundary_revision = number(&mut fields, "boundary_revision")?;
    require_tag(&mut fields, "overlap_policy", "none")?;
    require_tag(&mut fields, "anchor_policy", "exact-text-bytes")?;
    require_tag(&mut fields, "attachment_policy", "absent")?;
    require_tag(&mut fields, "omission_policy", "forbidden")?;
    require_tag(&mut fields, "empty_policy", "reject-empty")?;
    let mut limit_fields = closed(take(&mut fields, "limits")?)?;
    let limits = UnitizationLimits {
        max_input_bytes: size(&mut limit_fields, "max_input_bytes")?,
        preferred_unit_bytes: size(&mut limit_fields, "preferred_unit_bytes")?,
        max_unit_bytes: size(&mut limit_fields, "max_unit_bytes")?,
        max_lines: size(&mut limit_fields, "max_lines")?,
        max_units: size(&mut limit_fields, "max_units")?,
    };
    finish(limit_fields)?;
    let descriptor = V3UnitizerProfileDescriptor {
        profile_name,
        profile_revision,
        representation_kind,
        unit_kind,
        boundary_revision,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits,
        min_unit_bytes: size(&mut fields, "min_unit_bytes")?,
        max_unit_scalars: size(&mut fields, "max_unit_scalars")?,
        max_unit_lines: size(&mut fields, "max_unit_lines")?,
        max_anchor_depth: u8::try_from(number(&mut fields, "max_anchor_depth")?)
            .map_err(|_| UnitizationError::UnitizerProfileInvalid)?,
        max_manifest_bytes: size(&mut fields, "max_manifest_bytes")?,
        max_steps: number(&mut fields, "max_steps")?,
        fixture_digest: match take(&mut fields, "fixture_digest")? {
            CanonicalValue::Bytes(value) => Blake3Digest32::from_stored_bytes(
                value
                    .as_slice()
                    .try_into()
                    .map_err(|_| UnitizationError::UnitizerProfileInvalid)?,
            ),
            _ => return Err(UnitizationError::UnitizerProfileInvalid),
        },
    };
    finish(fields)?;
    validate_v3_unitizer_profile(&descriptor)
}

fn unsigned(value: usize) -> Result<CanonicalValue, UnitizationError> {
    u64::try_from(value)
        .map(CanonicalValue::U64)
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)
}

fn closed(value: CanonicalValue) -> Result<ClosedCanonicalObject, UnitizationError> {
    ClosedCanonicalObject::from_value(value, "v3_unitizer_profile")
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)
}

fn take(
    fields: &mut ClosedCanonicalObject,
    name: &'static str,
) -> Result<CanonicalValue, UnitizationError> {
    fields
        .take_required(name)
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)
}

fn finish(fields: ClosedCanonicalObject) -> Result<(), UnitizationError> {
    fields
        .finish()
        .map_err(|_| UnitizationError::UnitizerProfileInvalid)
}

fn read_text(
    fields: &mut ClosedCanonicalObject,
    name: &'static str,
) -> Result<String, UnitizationError> {
    match take(fields, name)? {
        CanonicalValue::Text(value) => Ok(value.into_string()),
        _ => Err(UnitizationError::UnitizerProfileInvalid),
    }
}

fn number(fields: &mut ClosedCanonicalObject, name: &'static str) -> Result<u64, UnitizationError> {
    match take(fields, name)? {
        CanonicalValue::U64(value) => Ok(value),
        _ => Err(UnitizationError::UnitizerProfileInvalid),
    }
}

fn size(fields: &mut ClosedCanonicalObject, name: &'static str) -> Result<usize, UnitizationError> {
    usize::try_from(number(fields, name)?).map_err(|_| UnitizationError::UnitizerProfileInvalid)
}

fn require_tag(
    fields: &mut ClosedCanonicalObject,
    name: &'static str,
    expected: &str,
) -> Result<(), UnitizationError> {
    if read_text(fields, name)? == expected {
        Ok(())
    } else {
        Err(UnitizationError::UnitizerProfileInvalid)
    }
}
