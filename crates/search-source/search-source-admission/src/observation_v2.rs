//! Pure relative bytes and immutable observation equality, without admission authority.
//!
//! This model neither resolves a native path nor implements a matcher. A missing
//! locator is explicit data; it must never be interpreted as a successful match.

use core::fmt;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit, blake3_canonical_array,
};

use super::{AdmissionError, AdmissionObservation};

/// Immutable byte-vocabulary profile; no normalization or native-path proof.
pub const RELATIVE_LOCATOR_PROFILE: &str = "eliot-admission-relative-utf8-portable@1";
/// Maximum bytes in one accepted locator, checked before copying.
pub const MAX_RELATIVE_LOCATOR_BYTES: usize = 4_096;
/// Maximum UTF-8 bytes in one component.
pub const MAX_RELATIVE_COMPONENT_BYTES: usize = 255;
/// Maximum components in one accepted locator.
pub const MAX_RELATIVE_COMPONENTS: usize = 128;
/// Distinct observation equality schema, never a v1 receipt schema.
pub const OBSERVATION_SCHEMA_VERSION: u32 = 2;
/// Shared CBOR/BLAKE3 domain for the complete frozen observation model.
pub const OBSERVATION_DIGEST_DOMAIN: &str = "eliot/cbor/admission/observation/v2";
/// Fixed complete domain-plus-CBOR preimage ceiling.
pub const MAX_OBSERVATION_PREIMAGE_BYTES: usize = 16_384;

const OBSERVATION_ROWS: usize = 15;

/// Owner-validated portable UTF-8 bytes relative to a logical root.
///
/// This is a byte vocabulary, not evidence of containment, source identity,
/// root registration, access, currentness or native name resolution.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmissionRelativeLocator {
    bytes: Vec<u8>,
    components: u16,
}

impl AdmissionRelativeLocator {
    /// Validates borrowed bytes before allocating or retaining them.
    ///
    /// Case and Unicode bytes remain exact. Absolute, traversal, nonportable
    /// native and malformed spellings are rejected rather than normalized.
    ///
    /// # Errors
    /// Returns the content-free `ADMISSION_OBSERVATION_INVALID` for any
    /// malformed, nonportable or over-limit input.
    pub fn from_bytes(input: &[u8]) -> Result<Self, AdmissionError> {
        if input.is_empty() || input.len() > MAX_RELATIVE_LOCATOR_BYTES {
            return Err(AdmissionError::ObservationInvalid);
        }
        let value = std::str::from_utf8(input).map_err(|_| AdmissionError::ObservationInvalid)?;
        if value.starts_with('/')
            || value.ends_with('/')
            || value.chars().any(char::is_control)
            || value
                .bytes()
                .any(|byte| matches!(byte, b'\\' | b':' | b'<' | b'>' | b'"' | b'|' | b'*' | b'?'))
        {
            return Err(AdmissionError::ObservationInvalid);
        }
        let mut components = 0_usize;
        for component in value.split('/') {
            components = components.saturating_add(1);
            if components > MAX_RELATIVE_COMPONENTS
                || component.is_empty()
                || component.len() > MAX_RELATIVE_COMPONENT_BYTES
                || matches!(component, "." | "..")
                || component.ends_with(['.', ' '])
                || native_device_name(component)
            {
                return Err(AdmissionError::ObservationInvalid);
            }
        }
        let components =
            u16::try_from(components).map_err(|_| AdmissionError::ObservationInvalid)?;
        Ok(Self {
            bytes: input.to_vec(),
            components,
        })
    }

    /// Exact validated bytes, suitable for a future byte-based matcher.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Finite component count; no native directory traversal was performed.
    #[must_use]
    pub const fn component_count(&self) -> u16 {
        self.components
    }
}

fn native_device_name(component: &str) -> bool {
    let base = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_matches(' ');
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|name| base.eq_ignore_ascii_case(name))
    {
        return true;
    }
    let Some(prefix) = base.get(..3) else {
        return false;
    };
    (prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT"))
        && matches!(
            base.get(3..),
            Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        )
}

impl fmt::Debug for AdmissionRelativeLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmissionRelativeLocator")
            .field("profile", &RELATIVE_LOCATOR_PROFILE)
            .field("bytes", &self.bytes.len())
            .field("components", &self.components)
            .finish()
    }
}

/// Internally computed BLAKE3 equality identity of the complete v2 model.
///
/// No public constructor accepts a digest. Equality grants no admission,
/// native-path, source-revision or completeness authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationFingerprintV2(Blake3Digest32);

impl ObservationFingerprintV2 {
    /// Computed digest bytes, distinct by type/domain/schema from v1 SHA bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

/// Immutable validated metadata plus an explicitly available or absent locator.
///
/// This is an observation model only. It creates no decision, receipt or policy
/// and supplies no v2-to-v1 fallback.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmissionObservationV2 {
    metadata: AdmissionObservation,
    relative_locator: Option<AdmissionRelativeLocator>,
    fingerprint: ObservationFingerprintV2,
}

impl AdmissionObservationV2 {
    /// Owns bounded validated inputs and computes their complete v2 identity.
    ///
    /// Metadata comes through its existing sole validator; the locator comes
    /// through this module's before-copy validator. Rows are constructed lazily
    /// from bounded fields, never through a local codec or caller fingerprint.
    ///
    /// # Errors
    /// Shared canonical/domain/ceiling failures return the content-free
    /// `ADMISSION_OBSERVATION_INVALID`, with no model or partial digest.
    pub fn new(
        metadata: AdmissionObservation,
        relative_locator: Option<AdmissionRelativeLocator>,
    ) -> Result<Self, AdmissionError> {
        let result = (|| {
            let domain = CanonicalDigestDomain::parse(OBSERVATION_DIGEST_DOMAIN)?;
            let limit = DigestInputLimit::new(MAX_OBSERVATION_PREIMAGE_BYTES)?;
            blake3_canonical_array(
                &domain,
                OBSERVATION_ROWS,
                (0..OBSERVATION_ROWS)
                    .map(|index| observation_row(&metadata, relative_locator.as_ref(), index)),
                limit,
            )
        })();
        let fingerprint = result.map_err(|_| AdmissionError::ObservationInvalid)?;
        Ok(Self {
            metadata,
            relative_locator,
            fingerprint: ObservationFingerprintV2(fingerprint),
        })
    }

    /// Existing bounded metadata, without mutable access or receipt authority.
    #[must_use]
    pub const fn metadata(&self) -> &AdmissionObservation {
        &self.metadata
    }

    /// Explicit locator availability; absence does not mean a match or allow.
    #[must_use]
    pub fn relative_locator(&self) -> Option<&AdmissionRelativeLocator> {
        self.relative_locator.as_ref()
    }

    /// Complete immutable observation equality identity.
    #[must_use]
    pub const fn fingerprint(&self) -> ObservationFingerprintV2 {
        self.fingerprint
    }
}

impl fmt::Debug for AdmissionObservationV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Metadata IDs and locator bytes are intentionally absent.
        formatter
            .debug_struct("AdmissionObservationV2")
            .field("schema", &OBSERVATION_SCHEMA_VERSION)
            .field("locator", &self.relative_locator)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

fn integer(value: usize) -> Result<CanonicalValue, ContractError> {
    u64::try_from(value).map(CanonicalValue::U64).map_err(|_| {
        ContractError::new(ContractErrorKind::InvalidRange, "admission_observation_v2")
    })
}

fn optional_text(value: Option<&str>) -> Result<CanonicalValue, ContractError> {
    value.map_or(Ok(CanonicalValue::Null), text)
}

fn optional_bool(value: Option<bool>) -> CanonicalValue {
    value.map_or(CanonicalValue::Null, CanonicalValue::Bool)
}

fn observation_row(
    metadata: &AdmissionObservation,
    locator: Option<&AdmissionRelativeLocator>,
    index: usize,
) -> Result<CanonicalValue, ContractError> {
    match index {
        0 => BoundedList::new(vec![
            CanonicalValue::U64(u64::from(OBSERVATION_SCHEMA_VERSION)),
            text(RELATIVE_LOCATOR_PROFILE)?,
            integer(MAX_RELATIVE_LOCATOR_BYTES)?,
            integer(MAX_RELATIVE_COMPONENT_BYTES)?,
            integer(MAX_RELATIVE_COMPONENTS)?,
            integer(MAX_OBSERVATION_PREIMAGE_BYTES)?,
        ])
        .map(CanonicalValue::Array),
        1 => text(metadata.locator_class.as_str()),
        2 => text(metadata.location_class.as_str()),
        3 => text(metadata.source_kind.as_str()),
        4 => text(metadata.source_class.as_str()),
        5 => Ok(metadata
            .byte_size
            .map_or(CanonicalValue::Null, CanonicalValue::U64)),
        6 => Ok(optional_bool(metadata.is_generated)),
        7 => Ok(optional_bool(metadata.is_vendor)),
        8 => Ok(optional_bool(metadata.is_binary)),
        9 => Ok(optional_bool(metadata.is_system)),
        10 => optional_text(metadata.sensitivity.map(super::SensitivityLevel::as_str)),
        11 => optional_text(metadata.detector_id.as_deref()),
        12 => optional_text(metadata.profile_id.as_deref()),
        13 => {
            let fields = metadata
                .unavailable_fields
                .iter()
                .map(|field| text(field))
                .collect::<Result<Vec<_>, _>>()?;
            BoundedList::new(fields).map(CanonicalValue::Array)
        }
        14 => locator.map_or(Ok(CanonicalValue::Null), |locator| {
            BoundedBytes::new(locator.as_bytes().to_vec()).map(CanonicalValue::Bytes)
        }),
        _ => Err(ContractError::new(
            ContractErrorKind::InvalidRange,
            "admission_observation_v2_row",
        )),
    }
}

// Private synthetic cases: AUTHORED_NOT_RUN under #352/#241/#215.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BASELINE_PROFILE, DEFAULT_ADMISSION_LIMITS, UnvalidatedObservationInput, baseline_policy,
        validate_observation,
    };
    use search_contracts::to_canonical_cbor;

    fn input() -> UnvalidatedObservationInput {
        UnvalidatedObservationInput {
            locator_class: "normalized-file".to_owned(),
            location_class: "local-fixed".to_owned(),
            source_kind: "file".to_owned(),
            source_class: "regular".to_owned(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".to_owned()),
            detector_id: Some("detector:baseline-v1".to_owned()),
            profile_id: Some(BASELINE_PROFILE.to_owned()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn metadata(input: &UnvalidatedObservationInput) -> AdmissionObservation {
        validate_observation(input, &baseline_policy(3, 1_024), DEFAULT_ADMISSION_LIMITS)
            .expect("synthetic metadata validates")
    }

    fn model(input: &UnvalidatedObservationInput, path: Option<&[u8]>) -> AdmissionObservationV2 {
        let locator = path.map(|bytes| AdmissionRelativeLocator::from_bytes(bytes).unwrap());
        AdmissionObservationV2::new(metadata(input), locator).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn locator_preserves_case_and_unicode_bytes_without_normalization() {
        for path in ["Src/é.rs", "src/e\u{301}.rs", "研究/資料.txt", "dir/FILE"] {
            let locator = AdmissionRelativeLocator::from_bytes(path.as_bytes()).unwrap();
            assert_eq!(locator.as_bytes(), path.as_bytes());
            assert_eq!(locator.component_count(), 2);
        }
        assert_ne!(
            AdmissionRelativeLocator::from_bytes("src/é.rs".as_bytes()).unwrap(),
            AdmissionRelativeLocator::from_bytes("src/e\u{301}.rs".as_bytes()).unwrap()
        );
        assert_ne!(
            AdmissionRelativeLocator::from_bytes(b"Src/lib.rs").unwrap(),
            AdmissionRelativeLocator::from_bytes(b"src/lib.rs").unwrap()
        );
    }

    #[test]
    fn malformed_traversal_native_and_device_aliases_are_rejected() {
        for path in [
            "",
            "/root",
            "dir/",
            "a//b",
            ".",
            "..",
            "a/./b",
            "a/../b",
            "C:/file",
            "C:file",
            "//host/share",
            "a\\b",
            "a\0b",
            "a\u{85}b",
            "a<b",
            "a>b",
            "a\"b",
            "a|b",
            "a*b",
            "a?b",
            "name.",
            "name ",
            "CON",
            "prn.txt",
            "Aux.rs",
            "NUL",
            "COM1",
            "com9.ext",
            "LPT1",
            "lpt9.ext",
            "COM¹",
            "LPT².txt",
            "COM³.ext",
            "CONIN$",
            "conout$.x",
            "clock$",
            "CON .txt",
            " CON.txt",
            "dir/AUX/file",
        ] {
            assert_eq!(
                AdmissionRelativeLocator::from_bytes(path.as_bytes()),
                Err(AdmissionError::ObservationInvalid)
            );
        }
        assert_eq!(
            AdmissionRelativeLocator::from_bytes(&[0xff]),
            Err(AdmissionError::ObservationInvalid)
        );
        for path in ["com0", "com10", "lpt0", ".CON", "console.txt"] {
            assert!(AdmissionRelativeLocator::from_bytes(path.as_bytes()).is_ok());
        }
    }

    #[test]
    fn each_locator_dimension_accepts_exact_limit_and_refuses_plus_one() {
        let mut components = vec!["a".repeat(255); 15];
        components.extend(["b".repeat(254), "c".to_owned()]);
        let exact = components.join("/");
        assert_eq!(exact.len(), MAX_RELATIVE_LOCATOR_BYTES);
        assert!(AdmissionRelativeLocator::from_bytes(exact.as_bytes()).is_ok());
        assert_eq!(
            AdmissionRelativeLocator::from_bytes(format!("{exact}d").as_bytes()),
            Err(AdmissionError::ObservationInvalid)
        );

        let component = "界".repeat(85);
        assert_eq!(component.len(), MAX_RELATIVE_COMPONENT_BYTES);
        assert!(AdmissionRelativeLocator::from_bytes(component.as_bytes()).is_ok());
        assert_eq!(
            AdmissionRelativeLocator::from_bytes(format!("{component}a").as_bytes()),
            Err(AdmissionError::ObservationInvalid)
        );

        let exact = vec!["a"; MAX_RELATIVE_COMPONENTS].join("/");
        assert_eq!(
            AdmissionRelativeLocator::from_bytes(exact.as_bytes())
                .unwrap()
                .component_count(),
            128
        );
        assert_eq!(
            AdmissionRelativeLocator::from_bytes(format!("{exact}/a").as_bytes()),
            Err(AdmissionError::ObservationInvalid)
        );
    }

    #[test]
    fn complete_identity_is_deterministic_and_binds_path_absence_and_metadata() {
        let original = input();
        let baseline = model(&original, Some(b"src/lib.rs"));
        assert_eq!(baseline, model(&original, Some(b"src/lib.rs")));
        assert_eq!(baseline.metadata(), &metadata(&original));
        for path in [
            Some(b"src/main.rs".as_slice()),
            Some(b"Src/lib.rs".as_slice()),
            None,
        ] {
            assert_ne!(baseline.fingerprint(), model(&original, path).fingerprint());
        }
        let edits: &[fn(&mut UnvalidatedObservationInput)] = &[
            |i| i.locator_class = "normalized-directory".to_owned(),
            |i| i.location_class = "local-removable".to_owned(),
            |i| i.source_kind = "directory".to_owned(),
            |i| i.source_class = "documentation".to_owned(),
            |i| i.byte_size = Some(11),
            |i| i.is_generated = Some(true),
            |i| i.is_vendor = Some(true),
            |i| i.is_binary = Some(true),
            |i| i.is_system = Some(true),
            |i| i.sensitivity = Some("internal".to_owned()),
            |i| i.detector_id = Some("detector:other".to_owned()),
            |i| i.profile_id = Some("profile:other".to_owned()),
            |i| {
                i.byte_size = None;
                i.unavailable_fields = vec!["byte-size".to_owned()];
            },
        ];
        for edit in edits {
            let mut changed = original.clone();
            edit(&mut changed);
            assert_ne!(
                baseline.fingerprint(),
                model(&changed, Some(b"src/lib.rs")).fingerprint()
            );
        }
    }

    #[test]
    fn fifteen_rows_match_independent_cbor_golden_and_bind_domain_and_schema() {
        let baseline = model(&input(), Some(b"src/lib.rs"));
        let rows = (0..15)
            .map(|index| {
                observation_row(baseline.metadata(), baseline.relative_locator(), index).unwrap()
            })
            .collect::<Vec<_>>();
        let value = CanonicalValue::Array(BoundedList::new(rows.clone()).unwrap());
        // Independent .NET encoder: 158 CBOR bytes, domain + NUL + CBOR = 194 bytes.
        let expected = concat!(
            "8f86027828656c696f742d61646d697373696f6e2d72656c61746976652d757466382d706f727461626c654031",
            "19100018ff18801940006f6e6f726d616c697a65642d66696c656b6c6f63616c2d66697865646466696c6567",
            "726567756c61720af4f4f4f4667075626c6963746465746563746f723a626173656c696e652d763170626173",
            "656c696e652d736166652d7631804a7372632f6c69622e7273"
        );
        let cbor = to_canonical_cbor(&value).unwrap();
        assert_eq!(cbor.as_slice().len(), 158);
        assert_eq!(hex(cbor.as_slice()), expected);
        assert_eq!(
            hex(baseline.fingerprint().as_bytes()),
            "c93f5823279641ff55300ca4b17698f15dcc5570a860acbe2b92a5a1fd737ac5"
        );
        let limit = DigestInputLimit::new(MAX_OBSERVATION_PREIMAGE_BYTES).unwrap();
        let other_domain =
            CanonicalDigestDomain::parse("eliot/cbor/admission/observation/v3").unwrap();
        assert_ne!(
            blake3_canonical_array(&other_domain, 15, rows.clone().into_iter().map(Ok), limit)
                .unwrap()
                .as_bytes(),
            baseline.fingerprint().as_bytes()
        );
        let mut changed = rows;
        let CanonicalValue::Array(header) = &changed[0] else {
            panic!("header array")
        };
        let mut header = header.as_slice().to_vec();
        header[0] = CanonicalValue::U64(3);
        changed[0] = CanonicalValue::Array(BoundedList::new(header).unwrap());
        let domain = CanonicalDigestDomain::parse(OBSERVATION_DIGEST_DOMAIN).unwrap();
        assert_ne!(
            blake3_canonical_array(&domain, 15, changed.into_iter().map(Ok), limit)
                .unwrap()
                .as_bytes(),
            baseline.fingerprint().as_bytes()
        );

        let mut absent = input();
        absent.byte_size = None;
        absent.is_generated = None;
        absent.is_vendor = None;
        absent.is_binary = None;
        absent.is_system = None;
        absent.sensitivity = None;
        absent.detector_id = None;
        absent.profile_id = None;
        absent.unavailable_fields = crate::CLOSED_UNAVAILABLE_FIELDS
            .iter()
            .rev()
            .map(|s| (*s).to_owned())
            .collect();
        let absent = model(&absent, None);
        for index in 5..=12 {
            assert_eq!(
                observation_row(absent.metadata(), None, index).unwrap(),
                CanonicalValue::Null
            );
        }
        assert_eq!(
            observation_row(absent.metadata(), None, 14).unwrap(),
            CanonicalValue::Null
        );
        let expected_fields = [
            "binary-flag",
            "byte-size",
            "detector-id",
            "generated-flag",
            "profile-id",
            "sensitivity",
            "system-flag",
            "vendor-flag",
        ];
        let expected_fields = CanonicalValue::Array(
            BoundedList::new(
                expected_fields
                    .into_iter()
                    .map(|s| CanonicalValue::Text(CanonicalText::new(s).unwrap()))
                    .collect(),
            )
            .unwrap(),
        );
        assert_eq!(
            observation_row(absent.metadata(), None, 13).unwrap(),
            expected_fields
        );
    }

    #[test]
    fn debug_and_errors_omit_synthetic_private_marker() {
        let marker = "synthetic-private-marker-409";
        let mut observed = input();
        observed.detector_id = Some(marker.to_owned());
        observed.profile_id = Some(marker.to_owned());
        let path = format!("{marker}/file.rs");
        let observed = model(&observed, Some(path.as_bytes()));
        assert!(!format!("{observed:?}").contains(marker));
        assert!(!format!("{:?}", observed.relative_locator().unwrap()).contains(marker));
        let invalid = format!("{marker}/../file.rs");
        let error = AdmissionRelativeLocator::from_bytes(invalid.as_bytes()).unwrap_err();
        assert_eq!(error, AdmissionError::ObservationInvalid);
        assert!(!format!("{error:?}").contains(marker));
        assert!(!error.to_string().contains(marker));
    }
}
