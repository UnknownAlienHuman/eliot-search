//! Pure historical-byte fixtures; not native or installed qualification.

use super::*;

fn charge(_: usize) -> Result<(), OwnerError> {
    Ok(())
}

fn subject() -> InitializationSubject {
    let mut value = InitializationSubject {
        original_request: [1; 16],
        request_digest: [0; 32],
        arguments: vec![
            native_literal("--initialize-data-root"),
            native_literal("C:\\fixture"),
            native_literal(&hex(&[2; 16])),
        ],
        initialization: [2; 16],
        installation: [3; 16],
        incarnation: [4; 16],
        physical_root: [5; 20],
        root_id: [6; 16],
        path_digest: [7; 32],
        volume_digest: [8; 32],
        executable: [9; 32],
        namespace: [0; 32],
        owner_token: [10; 16],
        owner_pid: 42,
        journal_identity: [11; 20],
    };
    value.request_digest = hash_rows(
        REQUEST_DOMAIN,
        std::iter::once(Ok(CanonicalValue::U64(2)))
            .chain(value.arguments.iter().map(|argument| bytes_value(argument))),
        4,
        MAX_BODY,
        &mut charge,
    )
    .unwrap();
    value.namespace = initialization_namespace(
        value.initialization,
        value.installation,
        value.incarnation,
        value.root_id,
        value.executable,
        &mut charge,
    )
    .unwrap();
    value
}

fn completion(subject: &InitializationSubject) -> InitializationCompletion {
    let mut identities = [[12; 20]; 11];
    for (index, identity) in identities.iter_mut().enumerate() {
        identity[0] = u8::try_from(index + 20).unwrap();
    }
    identities[0] = subject.physical_root;
    identities[10] = subject.journal_identity;
    let native_digest = native_objects_digest(&identities, &mut charge).unwrap();
    let mut record = DurableOwnerRecord {
        installation_id: subject.installation,
        installation_incarnation_id: subject.incarnation,
        data_root_id: subject.root_id,
        epoch: 1,
        previous_epoch: 0,
        previous_record_digest: [0; 32],
        canonical_path_digest: subject.path_digest,
        volume_identity_digest: subject.volume_digest,
        executable_digest: subject.executable,
        owner_token: subject.owner_token,
        owner_pid: subject.owner_pid,
        lifecycle: LifecycleState::Active,
        drain_reason: DrainReasonText::None,
        generation: 1,
        record_digest: [0; 32],
    };
    record.record_digest = blake3_bytes(&record.encode_body());
    let active = record.encode();
    record.lifecycle = LifecycleState::Draining;
    record.drain_reason = DrainReasonText::Shutdown;
    record.generation = 2;
    record.record_digest = blake3_bytes(&record.encode_body());
    let draining = record.encode();
    record.lifecycle = LifecycleState::Released;
    record.drain_reason = DrainReasonText::None;
    record.generation = 3;
    record.record_digest = blake3_bytes(&record.encode_body());
    InitializationCompletion {
        subject_digest: subject.digest(&mut charge).unwrap(),
        native_objects: identities,
        native_digest,
        installation_bytes: installation_bytes(subject, native_digest),
        namespace_bytes: format!("{}\n", hex(&subject.namespace)).into_bytes(),
        log_bytes: format!("{}\n", search_source_registry::LEGACY_DIRECT_LOG_HEADER).into_bytes(),
        lifecycle_bytes: [active, draining, record.encode()],
        selected_key_binding: [13; 32],
    }
}

fn complete_bytes(
    subject: &InitializationSubject,
    completion: &InitializationCompletion,
) -> Vec<u8> {
    let mut bytes = subject.frame().unwrap();
    bytes.extend_from_slice(&completion.frame().unwrap());
    bytes
}

#[test]
fn exact_original_and_single_historical_completion_roundtrip() {
    let subject = subject();
    let completion = completion(&subject);
    let bytes = complete_bytes(&subject, &completion);
    let decoded = decode_journal(&bytes, &mut charge).unwrap();
    assert!(decoded.subject == subject);
    assert!(decoded.completion == Some(completion));
    assert!(bytes.len() < MAX_JOURNAL);
}

#[test]
fn body_only_remains_distinct_from_completed_history() {
    let subject = subject();
    let decoded = decode_journal(&subject.frame().unwrap(), &mut charge).unwrap();
    assert!(decoded.subject == subject);
    assert!(decoded.completion.is_none());
}

#[test]
fn torn_duplicate_and_trailing_completion_are_refused() {
    let subject = subject();
    let completion = completion(&subject);
    let bytes = complete_bytes(&subject, &completion);
    for length in [0, MAGIC.len(), MAGIC.len() + 1, bytes.len() - 1] {
        assert!(decode_journal(&bytes[..length], &mut charge).is_err());
    }
    let mut duplicate = bytes.clone();
    duplicate.extend_from_slice(&completion.frame().unwrap());
    assert!(decode_journal(&duplicate, &mut charge).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode_journal(&trailing, &mut charge).is_err());
}

#[test]
fn stale_request_digest_refuses_changed_native_argv_or_namespace() {
    let original = subject();
    for index in 0..3 {
        let mut changed = original.clone();
        changed.arguments[index][0] ^= 1;
        assert!(changed.validate(&mut charge).is_err());
    }
    let mut changed = original;
    changed.namespace[0] ^= 1;
    assert!(changed.validate(&mut charge).is_err());
}

#[test]
fn seal_rejects_changed_subject_native_binding_and_lifecycle() {
    let subject = subject();
    let original = completion(&subject);
    let mut changed = original.clone();
    changed.subject_digest[0] ^= 1;
    assert!(changed.validate(&subject, &mut charge).is_err());
    changed = original.clone();
    changed.native_objects[4][0] ^= 1;
    assert!(changed.validate(&subject, &mut charge).is_err());
    changed = original.clone();
    changed.namespace_bytes[0] ^= 1;
    assert!(changed.validate(&subject, &mut charge).is_err());
    changed = original.clone();
    changed.log_bytes.push(b'\n');
    assert!(changed.validate(&subject, &mut charge).is_err());
    changed = original;
    changed.lifecycle_bytes.swap(1, 2);
    assert!(changed.validate(&subject, &mut charge).is_err());
}

#[test]
fn canonical_decoder_rejects_nonminimal_and_oversized_framing() {
    let subject = subject();
    let mut nonminimal = subject.frame().unwrap();
    let start = MAGIC.len() + 4;
    assert_eq!(nonminimal[start], 0x94); // Closed twenty-element array.
    nonminimal.splice(start..start + 1, [0x98, 20]);
    let length = u32::try_from(nonminimal.len() - start).unwrap();
    nonminimal[MAGIC.len()..start].copy_from_slice(&length.to_be_bytes());
    assert!(decode_journal(&nonminimal, &mut charge).is_err());
    for size in [MAX_BODY, MAX_BODY + 1] {
        let mut encoded = Vec::new();
        append_frame(&mut encoded, &vec![7; size]).unwrap();
        let mut remaining = encoded.as_slice();
        let original = remaining;
        let result = take_frame(&mut remaining, MAX_BODY);
        if size == MAX_BODY {
            assert_eq!(result.unwrap(), &vec![7; size]);
            assert!(remaining.is_empty());
        } else {
            assert_eq!(result, Err(OwnerError::OwnerRecoveryQuarantined));
            assert_eq!(remaining, original);
        }
    }
}

#[test]
fn exact_complete_hash_preimages_are_reserved_and_lower_total_refuses() {
    let subject = subject();
    let completion = completion(&subject);
    let bytes = complete_bytes(&subject, &completion);
    // Independent fixed preimages for the fixture above: domain + NUL + CBOR.
    let expected = [177, 152, 552, 271];
    let total = 1152_usize;
    let mut amounts = Vec::new();
    decode_journal(&bytes, &mut |amount| {
        amounts.push(amount);
        Ok(())
    })
    .unwrap();
    assert_eq!(amounts, expected);
    for ceiling in [total, total - 1] {
        let mut used = 0_usize;
        let result = decode_journal(&bytes, &mut |amount| {
            used = used
                .checked_add(amount)
                .ok_or(OwnerError::ContractExhausted)?;
            if used > ceiling {
                Err(OwnerError::ContractExhausted)
            } else {
                Ok(())
            }
        });
        if ceiling == total {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(OwnerError::ContractExhausted)));
        }
    }
    for refused in 0..expected.len() {
        let mut seen = Vec::new();
        let result = decode_journal(&bytes, &mut |amount| {
            seen.push(amount);
            if seen.len() == refused + 1 {
                Err(OwnerError::OwnerDeadlineInvalid)
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(OwnerError::OwnerDeadlineInvalid)));
        assert_eq!(seen, expected[..=refused]);
    }
}

#[test]
fn outbound_local_limits_refuse_before_hash_callback_or_frame() {
    let mut subject = subject();
    subject.arguments[1] = vec![9; 256 * 1024];
    let mut calls = 0;
    assert!(
        subject
            .digest(&mut |_| {
                calls += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(calls, 0);
    assert!(subject.frame().is_err());
    let subject = self::subject();
    for index in 0..6 {
        let mut seal = completion(&subject);
        let (value, ceiling) = match index {
            0 => (&mut seal.installation_bytes, 1024),
            1 => (&mut seal.namespace_bytes, 65),
            2 => (&mut seal.log_bytes, 30),
            other => (&mut seal.lifecycle_bytes[other - 3], 4096),
        };
        *value = vec![7; ceiling + 1];
        let mut calls = 0;
        assert!(
            seal.validate(&subject, &mut |_| {
                calls += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert!(seal.frame().is_err());
    }
}

#[test]
fn historical_semantics_refuse_fully_rehashed_alternatives() {
    let subject = subject();
    let original = completion(&subject);
    for index in [0, 10] {
        let mut seal = original.clone();
        seal.native_objects[index][0] ^= 1;
        seal.native_digest = native_objects_digest(&seal.native_objects, &mut charge).unwrap();
        seal.installation_bytes = installation_bytes(&subject, seal.native_digest);
        assert!(seal.validate(&subject, &mut charge).is_err());
    }
    for index in 0..3 {
        let mut seal = original.clone();
        let mut record = DurableOwnerRecord::decode(&seal.lifecycle_bytes[index]).unwrap();
        record.owner_token[0] ^= 1;
        record.refresh_digest();
        seal.lifecycle_bytes[index] = record.encode();
        assert!(seal.validate(&subject, &mut charge).is_err());
    }
    let mut seal = original.clone();
    let mut lines: Vec<_> = std::str::from_utf8(&seal.lifecycle_bytes[0])
        .unwrap()
        .lines()
        .collect();
    lines.swap(2, 3);
    let body = format!("{}\n", lines[..16].join("\n"));
    let changed = format!(
        "{body}record_digest={}\n",
        hex(&blake3_bytes(body.as_bytes()))
    )
    .into_bytes();
    assert!(DurableOwnerRecord::decode(&changed).is_ok());
    seal.lifecycle_bytes[0] = changed;
    assert!(seal.validate(&subject, &mut charge).is_err());
    let mut seal = original;
    seal.selected_key_binding = [0; 32];
    assert!(seal.validate(&subject, &mut charge).is_err());
}
