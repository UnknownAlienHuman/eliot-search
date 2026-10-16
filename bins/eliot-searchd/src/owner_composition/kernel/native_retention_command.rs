//! Bounded normal-CLI parsing for one explicit Source retention and one
//! retained-readback slice over the same DIRECT owner.
//!
//! This module owns only the two argument surfaces and their exact shape. It
//! opens nothing, rereads no config, takes no clock reading and allocates no
//! identity or key material; the qualified capture, journal, Native and object
//! owners stay behind the manager's dispatcher wiring. The derived
//! `native-retain:<id>` and `native-retention-namespace:<id>` values are
//! operation identities only: they are not UUID domains, not identity
//! allocation, and they grant no authority.

use super::source_command::NativeSourceCommand;
use search_contracts as c;
use search_source_admission::observation_v2::AdmissionRelativeLocator;
use std::ffi::OsString;

/// Actual retained AES profile plaintext ceiling for one readback slice.
const MAX_RETAINED_READ_BYTES: u64 = search_os_secrets::LEGACY_REVISION_MAX_PLAINTEXT_LENGTH;

/// `--retain-source-v2 DATA_ROOT SOURCE_ROOT RELATIVE POLICY_FILE OPERATION
/// PREDECESSOR REVISION`.
///
/// The three path arguments stay opaque captured bytes for the existing
/// qualified policy/root capture: they are never opened, canonicalized or
/// interpreted here. `predecessor` is the exact non-zero Source record
/// revision expected from the admission owner; `expected_revision` is the
/// separate full Source revision identity, which no record revision substitutes
/// for.
pub(super) struct NativeRetentionCommand {
    pub(super) data_root: OsString,
    pub(super) source_root: OsString,
    pub(super) policy_file: OsString,
    pub(super) relative: AdmissionRelativeLocator,
    pub(super) predecessor: Option<c::NonZeroRevision>,
    pub(super) expected_revision: Option<c::SourceRevisionId>,
    pub(super) operation: c::OpaqueId,
    pub(super) namespace_operation: c::OpaqueId,
}

impl NativeRetentionCommand {
    pub(super) fn admission(&self) -> Result<NativeSourceCommand, String> {
        let id = self.operation.as_str().strip_prefix("native-retain:")
            .ok_or("NATIVE_RETENTION_OPERATION_INVALID")?;
        Ok(NativeSourceCommand { relative: self.relative.clone(), predecessor: self.predecessor,
            operation: c::OpaqueId::new(format!("native-source:{id}"))
                .map_err(|_| "NATIVE_RETENTION_OPERATION_INVALID")? })
    }
    /// Exact eight-argument parse with no side effect beyond the two derived
    /// operation identities.
    ///
    /// The relative locator and the Source record predecessor are validated by
    /// the existing admitted-Source owner over the identical first seven
    /// arguments, and its refusal codes are passed through unchanged. Only the
    /// last argument is specific to retention.
    pub(super) fn parse(selected: &[OsString]) -> Result<Self, String> {
        if selected.len() != 8 || selected[0] != "--retain-source-v2" {
            return Err("NATIVE_RETENTION_USAGE".into());
        }
        let source = NativeSourceCommand::parse(&selected[..7])?;
        // The raw operation identity is reused verbatim: one caller-supplied
        // identity, two distinct derived owners, and never the admission
        // operation itself.
        let id = selected[5]
            .to_str()
            .ok_or("NATIVE_RETENTION_OPERATION_INVALID")?;
        if source.operation.as_str().strip_prefix("native-source:") != Some(id) {
            return Err("NATIVE_RETENTION_OPERATION_INVALID".into());
        }
        let operation = c::OpaqueId::new(format!("native-retain:{id}"))
            .map_err(|_| "NATIVE_RETENTION_OPERATION_INVALID")?;
        let namespace_operation =
            c::OpaqueId::new(format!("native-retention-namespace:{id}"))
                .map_err(|_| "NATIVE_RETENTION_OPERATION_INVALID")?;
        Ok(Self {
            data_root: selected[1].clone(),
            source_root: selected[2].clone(),
            policy_file: selected[4].clone(),
            relative: source.relative,
            predecessor: source.predecessor,
            expected_revision: expected_revision(&selected[7])?,
            operation,
            namespace_operation,
        })
    }
}

/// `--read-retained-source-v2 DATA_ROOT NAMESPACE SOURCE REVISION OFFSET
/// LENGTH`.
///
/// Every identity is the genuine full v4 UUID of its typed owner and the byte
/// range is finite, non-empty and already proven not to wrap. Parsing alone
/// reads no source byte and reserves no read permit.
pub(super) struct NativeRetainedReadCommand {
    pub(super) data_root: OsString,
    pub(super) namespace: c::SourceNamespaceId,
    pub(super) source: c::SourceId,
    pub(super) revision: c::SourceRevisionId,
    pub(super) offset: u64,
    pub(super) length: u64,
    /// Exclusive end, already proven to fit without saturation.
    pub(super) end: u64,
}

impl NativeRetainedReadCommand {
    /// Exact seven-argument parse with no filesystem or identity work.
    pub(super) fn parse(selected: &[OsString]) -> Result<Self, String> {
        if selected.len() != 7 || selected[0] != "--read-retained-source-v2" {
            return Err("NATIVE_RETAINED_READ_USAGE".into());
        }
        let namespace = c::SourceNamespaceId::from_bytes(uuid_v4(
            selected[2]
                .to_str()
                .ok_or("NATIVE_RETAINED_READ_NAMESPACE_INVALID")?,
            "NATIVE_RETAINED_READ_NAMESPACE_INVALID",
        )?);
        let source = c::SourceId::from_bytes(uuid_v4(
            selected[3]
                .to_str()
                .ok_or("NATIVE_RETAINED_READ_SOURCE_INVALID")?,
            "NATIVE_RETAINED_READ_SOURCE_INVALID",
        )?);
        let revision = c::SourceRevisionId::from_bytes(uuid_v4(
            selected[4]
                .to_str()
                .ok_or("NATIVE_RETAINED_READ_REVISION_INVALID")?,
            "NATIVE_RETAINED_READ_REVISION_INVALID",
        )?);
        let offset = super::super::super::codec::parse_u64(selected[5].to_str())
            .map_err(|_| "NATIVE_RETAINED_READ_RANGE_INVALID".to_owned())?;
        let length = super::super::super::codec::parse_u64(selected[6].to_str())
            .map_err(|_| "NATIVE_RETAINED_READ_RANGE_INVALID".to_owned())?;
        if length == 0 || length > MAX_RETAINED_READ_BYTES {
            return Err("NATIVE_RETAINED_READ_RANGE_INVALID".into());
        }
        let end = offset
            .checked_add(length)
            .ok_or("NATIVE_RETAINED_READ_RANGE_INVALID")?;
        Ok(Self {
            data_root: selected[1].clone(),
            namespace,
            source,
            revision,
            offset,
            length,
            end,
        })
    }
}

/// `new` or one exact full v4 Source revision identity.
fn expected_revision(argument: &OsString) -> Result<Option<c::SourceRevisionId>, String> {
    const INVALID: &str = "NATIVE_RETENTION_REVISION_INVALID";
    let value = argument.to_str().ok_or(INVALID)?;
    if value == "new" {
        return Ok(None);
    }
    Ok(Some(c::SourceRevisionId::from_bytes(uuid_v4(value, INVALID)?)))
}

/// Exact 32-lowercase-hex v4 UUID bytes, never padded, truncated or re-versioned.
fn uuid_v4(value: &str, code: &'static str) -> Result<[u8; 16], String> {
    let bytes = super::super::super::codec::parse_id16(Some(value)).map_err(|_| code)?;
    if bytes[6] & 0xf0 != 0x40 || bytes[8] & 0xc0 != 0x80 {
        return Err(code.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One exact v4 UUID in the canonical 32-lowercase-hex spelling.
    const V4: &str = "0123456789ab4def8123456789abcdef";
    /// The same identity in the hyphenated typed spelling, never a CLI input.
    const V4_TYPED: &str = "01234567-89ab-4def-8123-456789abcdef";
    const OP: &str = "0123456789abcdef0123456789abcdef";

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    /// Only the closed refusal code is observable: a parsed command owns no
    /// comparable state and never a Debug rendering of captured inputs.
    fn refusal<T>(result: Result<T, String>) -> String {
        match result {
            Ok(_) => panic!("input must be refused"),
            Err(error) => error,
        }
    }

    fn retain(revision: &str) -> Vec<OsString> {
        arguments(&[
            "--retain-source-v2",
            "C:/data",
            "C:/source",
            "dir/file.txt",
            "C:/policy.cbor",
            OP,
            "new",
            revision,
        ])
    }

    fn read(namespace: &str, source: &str, revision: &str, offset: &str, length: &str) -> Vec<OsString> {
        arguments(&[
            "--read-retained-source-v2",
            "C:/data",
            namespace,
            source,
            revision,
            offset,
            length,
        ])
    }

    #[test]
    fn real_history_preseal_object_is_preserved_in_selected_crypto_aad() {
        use search_source_history::prepare::{SourceRevisionPresealProfile, encode_source_revision_preseal_profile};
        let owner_array = c::to_canonical_cbor(&c::CanonicalValue::Array(
            c::BoundedList::new(vec![c::CanonicalValue::U64(1)]).unwrap())).unwrap().into_vec();
        let uuid = |seed| {
            let mut bytes = [seed; 16]; bytes[6] = (bytes[6] & 15) | 64; bytes[8] = (bytes[8] & 63) | 128; bytes
        };
        let profile = SourceRevisionPresealProfile {
            operation_id: c::OpaqueId::new("fixture:real-preseal-map").unwrap(),
            source_namespace_id: c::SourceNamespaceId::from_bytes(uuid(1)),
            source_id: c::SourceId::from_bytes(uuid(2)), expected_head: None,
            revision_id: c::SourceRevisionId::from_bytes(uuid(3)), occurrence_sequence: 1,
            content_digest: c::Blake3Digest32::from_bytes([4; 32]), byte_length: 4,
            observed_at: c::UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").unwrap(),
            acquisition_kind: c::AcquisitionKind::Filesystem,
            stability_receipt_ref: c::ReceiptRef::new("fixture:stable-source").unwrap(),
            residency_closure: c::OpaqueCanonicalBytes::from_validated(owner_array.clone()).unwrap(),
        };
        let occurrence = encode_source_revision_preseal_profile(&profile).unwrap();
        assert!(matches!(c::parse_canonical_cbor(&occurrence).unwrap(), c::CanonicalValue::Object(_)));
        // Only the occurrence interoperability is exercised here; this fixture
        // creates no Current, source admission, key-use or access authority.
        let binding = search_revision_crypto::SelectedRevisionBindingV2::from_owner_records(
            &owner_array, &occurrence, &owner_array, &profile.operation_id, 0,
            profile.content_digest, profile.byte_length).expect("real C04 owner record must be admissible");
        let c::CanonicalValue::Array(aad) = c::parse_canonical_cbor(binding.aad_bytes()).unwrap() else { panic!("AAD array"); };
        let c::CanonicalValue::Bytes(retained) = &aad.as_slice()[5] else { panic!("exact owner bytes"); };
        assert_eq!(retained.as_slice(), occurrence);
        let scalar = c::to_canonical_cbor(&c::CanonicalValue::U64(1)).unwrap();
        assert!(search_revision_crypto::SelectedRevisionBindingV2::from_owner_records(
            &owner_array, scalar.as_slice(), &owner_array, &profile.operation_id, 0,
            profile.content_digest, profile.byte_length).is_err());
    }

    #[test]
    fn retention_parse_reuses_admission_and_derives_two_distinct_operations() {
        let command = NativeRetentionCommand::parse(&retain(V4)).expect("valid retention input");
        assert_eq!(command.relative.as_bytes(), b"dir/file.txt");
        assert_eq!(command.predecessor, None);
        assert_eq!(
            command.expected_revision,
            Some(c::SourceRevisionId::parse(V4_TYPED).expect("v4 revision"))
        );
        assert_eq!(command.operation.as_str(), format!("native-retain:{OP}"));
        assert_eq!(
            command.namespace_operation.as_str(),
            format!("native-retention-namespace:{OP}")
        );
        assert_ne!(command.operation, command.namespace_operation);
        assert_ne!(
            command.operation.as_str(),
            format!("native-source:{OP}"),
            "retention is not the admission operation"
        );
        // Opaque capture inputs survive byte for byte.
        assert_eq!(command.data_root, OsString::from("C:/data"));
        assert_eq!(command.source_root, OsString::from("C:/source"));
        assert_eq!(command.policy_file, OsString::from("C:/policy.cbor"));
        assert_eq!(
            NativeRetentionCommand::parse(&retain("new"))
                .expect("new revision")
                .expected_revision,
            None
        );
    }

    #[test]
    fn retention_parse_refuses_shape_padding_and_non_v4_revision() {
        for invalid in [
            "0".repeat(31),
            "0".repeat(33),
            V4.to_uppercase(),
            format!("{V4}00"),
            "0".repeat(32),
            format!("0{V4}"),
            "new ".to_owned(),
        ] {
            assert!(
                NativeRetentionCommand::parse(&retain(invalid.as_str())).is_err(),
                "{invalid} must be refused"
            );
        }
        let mut short = retain(V4);
        short.pop();
        assert_eq!(
            refusal(NativeRetentionCommand::parse(&short)),
            "NATIVE_RETENTION_USAGE"
        );
        let mut bad_operation = retain(V4);
        bad_operation[5] = OsString::from(OP.to_uppercase());
        assert_eq!(
            refusal(NativeRetentionCommand::parse(&bad_operation)),
            "NATIVE_SOURCE_OPERATION_INVALID"
        );
        let mut bad_predecessor = retain(V4);
        bad_predecessor[6] = OsString::from("0");
        assert_eq!(
            refusal(NativeRetentionCommand::parse(&bad_predecessor)),
            "NATIVE_SOURCE_PREDECESSOR_INVALID"
        );
    }

    #[test]
    fn retained_read_parse_is_exact_bounded_and_wrap_checked() {
        let command = NativeRetainedReadCommand::parse(&read(V4, V4, V4, "16", "1048576"))
            .expect("valid read input");
        assert_eq!(command.offset, 16);
        assert_eq!(command.length, 1_048_576);
        assert_eq!(command.end, 1_048_592);
        assert_eq!(command.namespace, c::SourceNamespaceId::parse(V4_TYPED).expect("v4"));
        assert_eq!(command.source, c::SourceId::parse(V4_TYPED).expect("v4"));
        assert_eq!(command.revision, c::SourceRevisionId::parse(V4_TYPED).expect("v4"));
        // The accepted ceiling is the actual retained AES profile plaintext cap.
        let ceiling = NativeRetainedReadCommand::parse(&read(V4, V4, V4, "0", "67108864"))
            .expect("the profile plaintext cap is admissible");
        assert_eq!(ceiling.length, MAX_RETAINED_READ_BYTES);
        assert_eq!(ceiling.end, MAX_RETAINED_READ_BYTES);
        let maximum = u64::MAX.to_string();
        for (offset, length) in [
            ("0", "0"),
            ("0", "67108865"),
            ("0", maximum.as_str()),
            (maximum.as_str(), "1"),
            ("+1", "1"),
            ("01", "1"),
        ] {
            assert_eq!(
                refusal(NativeRetainedReadCommand::parse(&read(V4, V4, V4, offset, length))),
                "NATIVE_RETAINED_READ_RANGE_INVALID",
                "{offset}/{length} must be refused"
            );
        }
        assert_eq!(
            refusal(NativeRetainedReadCommand::parse(&read("0", V4, V4, "0", "1"))),
            "NATIVE_RETAINED_READ_NAMESPACE_INVALID"
        );
        assert_eq!(
            refusal(NativeRetainedReadCommand::parse(&read(V4, V4, V4, "0", "1")[..6])),
            "NATIVE_RETAINED_READ_USAGE"
        );
    }
}
