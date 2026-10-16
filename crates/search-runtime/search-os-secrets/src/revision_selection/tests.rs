//! Pure metadata proofs; no Native, credential, clock or journal effects.
use super::*;
use crate::{RevisionDekBinding, RevisionDekIntent};
use search_config as cfg;

fn source(kind: cfg::ConfigSourceKind) -> cfg::ConfigSource {
    cfg::ConfigSource {
        kind,
        source_ref: cfg::ConfigSourceRef::new("selection-fixture", 128).unwrap(),
        source_digest: c::blake3_bounded_bytes(
            &[kind.precedence()],
            c::Blake3ByteBudget::new(1).unwrap(),
        )
        .unwrap(),
    }
}
fn config(ttl: i64) -> (ValidatedSecretsConfig, ConfigFingerprint) {
    let descriptor = crate::section_descriptor().unwrap();
    let limits = cfg::ConfigLimits::W1;
    let profile = c::ProfileId::new("direct").unwrap();
    let path = cfg::ConfigKeyPath::new(
        descriptor.name().clone(),
        cfg::ConfigKeyName::new("lease_ttl_ms", 128).unwrap(),
    );
    let file = cfg::ConfigDocument::from_entries(
        2,
        Some(profile.clone()),
        source(cfg::ConfigSourceKind::File),
        [(
            path,
            cfg::LayerOperation::Set(cfg::DocumentValue::Integer(ttl)),
        )],
        limits,
    )
    .unwrap();
    let registry = cfg::register_sections(2, [descriptor.clone()], limits).unwrap();
    let merged = cfg::merge_layers(
        cfg::ConfigLayers {
            defaults: source(cfg::ConfigSourceKind::Defaults),
            requested_profile: profile.clone(),
            file: Some(file),
            environment: None,
            cli: None,
        },
        &registry,
        limits,
    )
    .unwrap();
    let section = cfg::ValidatedSection::new(
        cfg::project_section(&merged, &descriptor).unwrap(),
        profile.clone(),
        limits,
    )
    .unwrap();
    let checked = crate::validate_secrets_section(&section).unwrap();
    let snapshot = cfg::assemble_effective(&registry, [section], profile, limits).unwrap();
    (checked, snapshot.fingerprint())
}
fn fixture_policy() -> RevisionDekSelectionPolicy {
    let (config, fingerprint) = config(30_000);
    RevisionDekSelectionPolicy::from_config(&config, fingerprint).unwrap()
}
fn candidate(axis: usize, creator_epoch: u64, protected: &[u8]) -> RevisionDekCandidate {
    let mut root = [1; 16];
    let mut install = [2; 16];
    let mut incarnation = [3; 16];
    let mut user = [4; 32];
    let mut operation = [5; 16];
    match axis {
        1 => root[0] ^= 1,
        2 => install[0] ^= 1,
        3 => incarnation[0] ^= 1,
        4 => user[0] ^= 1,
        5 => operation[0] ^= 1,
        _ => {}
    }
    let binding = RevisionDekBinding::new(
        c::DataRootId::from_bytes(root),
        c::InstallationId::from_bytes(install),
        c::InstallationIncarnationId::from_bytes(incarnation),
        user,
        operation,
    )
    .unwrap();
    let intent = RevisionDekIntent::new(
        binding.clone(),
        c::OwnerEpoch::new(creator_epoch).unwrap(),
        protected,
    )
    .unwrap();
    RevisionDekCandidate::new(&intent, &binding, protected).unwrap()
}
fn intent(epoch: u64) -> RevisionDekApplicationIntent {
    RevisionDekApplicationIntent::new(
        candidate(0, 7, b"protected-fixture"),
        [9; 16],
        fixture_policy(),
        c::OwnerEpoch::new(epoch).unwrap(),
    )
    .unwrap()
}
fn encoded(value: &V) -> Vec<u8> {
    c::to_canonical_cbor(value).unwrap().as_slice().to_vec()
}
fn record_value(current: bool) -> V {
    let original = intent(11);
    let record = if current {
        RevisionDekSelectionRecord::Current(RevisionDekCurrent::new(&original).unwrap())
    } else {
        RevisionDekSelectionRecord::Intent(original)
    };
    c::parse_canonical_cbor(&encode_revision_dek_selection_record(&record).unwrap()).unwrap()
}
fn mutate_array(value: &mut V, index: usize, replacement: V) {
    let V::Array(rows) = value else {
        panic!("fixture array");
    };
    let mut changed = rows.clone().into_vec();
    changed[index] = replacement;
    *value = array(changed).unwrap();
}
fn intent_body(value: &V) -> V {
    fields::<4>(value.clone()).unwrap()[3].clone()
}
fn nested_candidate(value: &V) -> V {
    let body = fields::<8>(intent_body(value)).unwrap();
    let V::Bytes(bytes) = &body[1] else {
        panic!("candidate byte string");
    };
    c::parse_canonical_cbor(bytes.as_slice()).unwrap()
}

#[test]
fn source_checked_policy_keeps_exact_v2_identity_without_applying_other_settings() {
    let (checked, fingerprint) = config(30_000);
    let policy = RevisionDekSelectionPolicy::from_config(&checked, fingerprint).unwrap();
    assert_eq!(policy.backend(), "windows_user_bound");
    assert_eq!(policy.config_fingerprint(), *fingerprint.as_bytes());
    assert_eq!(
        policy.secrets_section_digest(),
        *crate::section_digest(&checked).as_bytes()
    );
    assert_eq!(
        policy.registry_digest(),
        *crate::section_descriptor()
            .unwrap()
            .field_registry_digest()
            .as_bytes()
    );
    assert_eq!(
        RevisionDekSelectionPolicy::from_config(
            &checked,
            ConfigFingerprint::restore_legacy_v1(*fingerprint.as_bytes())
        ),
        Err(RevisionDekSelectionError::InvalidPolicy)
    );
    let (shorter, changed_fingerprint) = config(20_000);
    let changed = RevisionDekSelectionPolicy::from_config(&shorter, changed_fingerprint).unwrap();
    assert_ne!(changed.config_fingerprint(), policy.config_fingerprint());
    assert_ne!(
        changed.secrets_section_digest(),
        policy.secrets_section_digest()
    );
    let candidate = candidate(0, 7, b"protected-fixture");
    assert_eq!(
        changed.expected_backend_identity(&candidate),
        policy.expected_backend_identity(&candidate)
    );
}

#[test]
fn exact_candidate_v1_bytes_and_distinct_revision_axes_round_trip_both_stages() {
    let intent = intent(11);
    let candidate_bytes_before = candidate_bytes(intent.candidate()).unwrap();
    let current = RevisionDekCurrent::new(&intent).unwrap();
    assert_eq!(
        (
            current.applied_policy_revision(),
            current.selection_revision()
        ),
        (1, 1)
    );
    assert_eq!(current.intent(), &intent);
    for (kind, record) in [
        (0, RevisionDekSelectionRecord::Intent(intent.clone())),
        (1, RevisionDekSelectionRecord::Current(current)),
    ] {
        let bytes = encode_revision_dek_selection_record(&record).unwrap();
        assert!(bytes.len() <= REVISION_DEK_SELECTION_MAX_RECORD_BYTES);
        assert_eq!(bytes[0], 0x84); // Exact canonical four-array, not a renamed candidate record.
        let [marker, version, actual_kind, body] =
            fields::<4>(c::parse_canonical_cbor(&bytes).unwrap()).unwrap();
        assert_eq!(
            (marker, version, actual_kind),
            (
                text("eliot.revision-dek.selection.v1").unwrap(),
                V::U64(1),
                V::U64(kind)
            )
        );
        let body = if kind == 0 {
            body
        } else {
            fields::<4>(body).unwrap()[0].clone()
        };
        let [
            operation,
            candidate,
            config,
            section,
            registry,
            backend,
            scope,
            epoch,
        ] = fields::<8>(body).unwrap();
        assert_eq!(operation, super::bytes(&[9; 16]).unwrap());
        assert_eq!(candidate, super::bytes(&candidate_bytes_before).unwrap());
        assert_eq!(
            (config, section, registry),
            (
                super::bytes(&intent.policy().config_fingerprint()).unwrap(),
                super::bytes(&intent.policy().secrets_section_digest()).unwrap(),
                super::bytes(&intent.policy().registry_digest()).unwrap()
            )
        );
        assert_eq!(
            (backend, scope, epoch),
            (
                text("windows_user_bound").unwrap(),
                text("revision-dek-current-selection/windows-user-bound/v1").unwrap(),
                V::U64(11)
            )
        );
        assert_eq!(
            decode_revision_dek_selection_record(&bytes),
            Ok(record.clone())
        );
        assert_eq!(
            encode_revision_dek_selection_record(
                &decode_revision_dek_selection_record(&bytes).unwrap()
            )
            .unwrap(),
            bytes
        );
        assert_eq!(
            record.intent().candidate().intent().creator_epoch().get(),
            7
        );
    }
    assert_eq!(
        candidate_bytes(intent.candidate()).unwrap(),
        candidate_bytes_before
    );
}

#[test]
fn request_replay_excludes_application_creator_epoch_but_binds_each_logical_input() {
    let original = intent(11);
    let reopened = intent(12);
    assert_eq!(original.request_digest(), reopened.request_digest());
    assert_ne!(original.creator_epoch(), reopened.creator_epoch());
    assert_ne!(
        encode_revision_dek_selection_record(&RevisionDekSelectionRecord::Intent(original.clone()))
            .unwrap(),
        encode_revision_dek_selection_record(&RevisionDekSelectionRecord::Intent(reopened))
            .unwrap()
    );
    let epoch = c::OwnerEpoch::new(11).unwrap();
    let changed_operation = RevisionDekApplicationIntent::new(
        original.candidate().clone(),
        [10; 16],
        original.policy().clone(),
        epoch,
    )
    .unwrap();
    assert_ne!(
        original.request_digest(),
        changed_operation.request_digest()
    );
    let (checked, fingerprint) = config(20_000);
    let changed_policy = RevisionDekSelectionPolicy::from_config(&checked, fingerprint).unwrap();
    for changed in [
        policy(
            changed_policy.config_fingerprint(),
            original.policy().secrets_section_digest(),
            original.policy().registry_digest(),
        )
        .unwrap(),
        policy(
            original.policy().config_fingerprint(),
            changed_policy.secrets_section_digest(),
            original.policy().registry_digest(),
        )
        .unwrap(),
    ] {
        assert_ne!(
            original.request_digest(),
            RevisionDekApplicationIntent::new(
                original.candidate().clone(),
                [9; 16],
                changed,
                epoch
            )
            .unwrap()
            .request_digest()
        );
    }
    let mut alternatives = (1..=5)
        .map(|axis| candidate(axis, 7, b"protected-fixture"))
        .collect::<Vec<_>>();
    alternatives.push(candidate(0, 8, b"protected-fixture"));
    alternatives.push(candidate(0, 7, b"changed-protected-fixture"));
    alternatives.push(candidate(0, 7, b"protected-fixture!"));
    for changed in alternatives {
        assert_ne!(
            original
                .policy()
                .expected_backend_identity(&changed)
                .unwrap(),
            original
                .policy()
                .expected_backend_identity(original.candidate())
                .unwrap()
        );
        assert_ne!(
            original.request_digest(),
            RevisionDekApplicationIntent::new(changed, [9; 16], original.policy().clone(), epoch)
                .unwrap()
                .request_digest()
        );
    }
    for operation in [
        [0; 16],
        original.candidate().intent().binding().operation_id(),
    ] {
        assert_eq!(
            RevisionDekApplicationIntent::new(
                original.candidate().clone(),
                operation,
                original.policy().clone(),
                epoch
            ),
            Err(RevisionDekSelectionError::InvalidIntent)
        );
    }
}

#[test]
fn closed_policy_shape_nonzero_scope_and_revision_mismatches_refuse() {
    let original = record_value(false);
    for count in [3, 5] {
        let mut rows = fields::<4>(original.clone()).unwrap().to_vec();
        rows.resize(count, V::Null);
        assert!(decode_revision_dek_selection_record(&encoded(&array(rows).unwrap())).is_err());
    }
    for (index, replacement) in [
        (0, V::Null),
        (1, V::U64(0)),
        (1, V::U64(2)),
        (2, V::U64(2)),
        (2, V::I64(-1)),
        (3, V::Null),
    ] {
        let mut changed = original.clone();
        mutate_array(&mut changed, index, replacement);
        assert!(decode_revision_dek_selection_record(&encoded(&changed)).is_err());
    }
    let body = intent_body(&original);
    for (index, replacement) in [
        (0, bytes(&[0; 16]).unwrap()),
        (0, bytes(&[9; 15]).unwrap()),
        (1, V::Null),
        (2, bytes(&[0; 32]).unwrap()),
        (2, bytes(&[1; 31]).unwrap()),
        (3, bytes(&[0; 32]).unwrap()),
        (4, bytes(&[0; 32]).unwrap()),
        (4, bytes(&[77; 32]).unwrap()),
        (5, text("other-backend").unwrap()),
        (6, text("other-consumer").unwrap()),
        (7, V::U64(0)),
    ] {
        let mut changed_body = body.clone();
        mutate_array(&mut changed_body, index, replacement);
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, changed_body);
        assert!(
            decode_revision_dek_selection_record(&encoded(&changed)).is_err(),
            "body field {index}"
        );
    }
    for count in [7, 9] {
        let mut rows = fields::<8>(body.clone()).unwrap().to_vec();
        rows.resize(count, V::Null);
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, array(rows).unwrap());
        assert!(decode_revision_dek_selection_record(&encoded(&changed)).is_err());
    }
    let original = record_value(true);
    let body = intent_body(&original);
    for count in [3, 5] {
        let mut rows = fields::<4>(body.clone()).unwrap().to_vec();
        rows.resize(count, V::Null);
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, array(rows).unwrap());
        assert!(decode_revision_dek_selection_record(&encoded(&changed)).is_err());
    }
    for (index, replacement) in [
        (1, V::U64(0)),
        (1, V::U64(2)),
        (2, V::U64(0)),
        (2, V::U64(2)),
        (3, bytes(&[0; 32]).unwrap()),
        (3, bytes(&[88; 32]).unwrap()),
    ] {
        let mut changed_body = body.clone();
        mutate_array(&mut changed_body, index, replacement);
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, changed_body);
        assert!(decode_revision_dek_selection_record(&encoded(&changed)).is_err());
    }
}

#[test]
fn embedded_candidate_uses_existing_closed_codec_and_requires_candidate_stage() {
    let original = record_value(false);
    let original_candidate = nested_candidate(&original);
    let original_body = intent_body(&original);
    let binding = fields::<7>(original_candidate.clone()).unwrap()[3].clone();
    for (index, replacement) in [
        (0, bytes(&[0; 16]).unwrap()),
        (1, bytes(&[0; 16]).unwrap()),
        (2, bytes(&[0; 16]).unwrap()),
        (3, bytes(&[0; 32]).unwrap()),
        (4, bytes(&[0; 16]).unwrap()),
        (5, bytes(&[0; 32]).unwrap()),
        (6, V::U64(2)),
        (7, V::U64(2)),
        (8, text("foreign-purpose").unwrap()),
        (9, text("foreign-backend").unwrap()),
        (10, V::U64(2)),
    ] {
        let mut changed_binding = binding.clone();
        mutate_array(&mut changed_binding, index, replacement);
        let mut changed_candidate = original_candidate.clone();
        mutate_array(&mut changed_candidate, 3, changed_binding);
        let mut body = original_body.clone();
        mutate_array(&mut body, 1, bytes(&encoded(&changed_candidate)).unwrap());
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, body);
        assert!(
            decode_revision_dek_selection_record(&encoded(&changed)).is_err(),
            "candidate binding field {index}"
        );
    }
    for (index, replacement) in [
        (1, V::U64(2)),
        (2, V::U64(0)),
        (4, V::U64(0)),
        (5, bytes(&[0; 32]).unwrap()),
        (6, V::U64(0)),
        (6, V::U64(2517)),
    ] {
        let mut changed_candidate = original_candidate.clone();
        mutate_array(&mut changed_candidate, index, replacement);
        let mut body = original_body.clone();
        mutate_array(&mut body, 1, bytes(&encoded(&changed_candidate)).unwrap());
        let mut changed = original.clone();
        mutate_array(&mut changed, 3, body);
        assert!(decode_revision_dek_selection_record(&encoded(&changed)).is_err());
    }
}

#[test]
fn canonical_truncation_trailing_and_record_ceiling_are_closed() {
    for current in [false, true] {
        let value = record_value(current);
        let encoded = encoded(&value);
        for end in 0..encoded.len() {
            assert!(decode_revision_dek_selection_record(&encoded[..end]).is_err());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_revision_dek_selection_record(&trailing).is_err());
        let mut noncanonical = encoded.clone();
        let marker_bytes = c::to_canonical_cbor(&text(MARKER).unwrap()).unwrap();
        let codec_offset = 1 + marker_bytes.as_slice().len();
        assert_eq!(noncanonical[codec_offset], 1);
        drop(noncanonical.splice(codec_offset..=codec_offset, [0x18, 1]));
        assert!(decode_revision_dek_selection_record(&noncanonical).is_err());
    }
    assert!(
        decode_revision_dek_selection_record(&vec![0; REVISION_DEK_SELECTION_MAX_RECORD_BYTES + 1])
            .is_err()
    );
}

#[test]
fn public_debug_and_errors_never_claim_effect_or_expose_candidate_coordinates() {
    let intent = intent(11);
    let current = RevisionDekCurrent::new(&intent).unwrap();
    let debug = format!(
        "{:?} {intent:?} {current:?} {:?}",
        intent.policy(),
        RevisionDekSelectionRecord::Current(current.clone())
    );
    assert!(!debug.contains("protected-fixture"));
    assert!(!debug.contains("windows_user_bound"));
    assert!(!debug.contains("user_scope_digest"));
    assert!(!debug.contains("config_fingerprint"));
    for error in [
        RevisionDekSelectionError::InvalidPolicy,
        RevisionDekSelectionError::InvalidIntent,
        RevisionDekSelectionError::InvalidRecord,
        RevisionDekSelectionError::Encoding,
    ] {
        assert_eq!(error.to_string(), error.code());
        assert!(!format!("{error} {error:?}").contains("protected-fixture"));
    }
}
