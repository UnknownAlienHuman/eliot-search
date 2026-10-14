//! Same-owner component fixtures with injected native identities, not product qualification.
use super::super::super::tests::{Platform, Scratch, binding, deadline, fixture_digest};
use super::super::tests::{counts, map_binding, rows, source_map};
use super::*;
use crate::MigrationBootstrap;
use crate::migration::{
    LegacyInventoryEncoder, LegacyInventoryFamily as Family, LegacyInventoryStatus as Status,
    SourceContentManifestEncoder, SourceContentManifestHeader, SourceContentObjectReadback,
    SourceImportRecordArtifact, SourceImportRow,
};

fn okay(_: Work) -> Result<(), ()> {
    Ok(())
}

fn stream(events: &[SourceImportRow]) -> Vec<Vec<u8>> {
    stream_profile(events, crate::migration::LegacyInventoryProfile::V1)
}

fn stream_profile(events: &[SourceImportRow], profile: crate::migration::LegacyInventoryProfile) -> Vec<Vec<u8>> {
    let observed = binding();
    let mut encoder = LegacyInventoryEncoder::with_profile(LegacyInventoryLimits::BASELINE, profile).unwrap();
    let mut result = vec![
        encoder
            .header(LegacyInventoryBinding {
                owner: observed.owner,
                target: observed.target,
                namespace: observed.legacy_namespace,
                snapshot: observed.catalog_snapshot,
                registration: observed.registration,
            })
            .unwrap(),
    ];
    for family in Family::ALL {
        let terminal = if family == Family::Sources {
            for event in events {
                result.push(encoder.source_event(event).unwrap());
            }
            Status::Present
        } else {
            Status::AbsentProven
        };
        result.push(encoder.end_family(family, terminal, true).unwrap());
    }
    result.push(encoder.finish().unwrap().0);
    result
}

fn mapped(scratch: &Scratch, inventory: &[Vec<u8>]) -> OwnerSourceMappedReceipt {
    mapped_profile(scratch, inventory, crate::migration::LegacyInventoryProfile::V1)
}

fn mapped_profile(scratch: &Scratch, inventory: &[Vec<u8>], profile: crate::migration::LegacyInventoryProfile) -> OwnerSourceMappedReceipt {
    let mut bootstrap =
        MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest, okay)
            .unwrap();
    let platform = Platform::default();
    let mut writer = SourceImportRecordArtifact::create(
        &scratch.0,
        ".source-map.inventory-stage.tmp",
        platform.clone(),
        deadline(),
    )
    .unwrap();
    for row in inventory {
        writer.push(row, deadline()).unwrap();
    }
    let frozen = writer.freeze(deadline()).unwrap();
    let name = format!(
        "{}.{}",
        crate::persistent::bootstrap::codec::hex(frozen.chain()), profile.artifact_suffix()
    );
    let mut compare = frozen.begin_readback(deadline()).unwrap();
    for row in inventory {
        compare.compare(row, deadline()).unwrap();
    }
    let mut publication = compare
        .finish(deadline())
        .unwrap()
        .prepare_publication(&name, deadline())
        .unwrap();
    bootstrap
        .publish_inventory(&mut publication, &platform, deadline(), okay)
        .unwrap();
    let mut source = source_map(scratch, platform.clone());
    let mut stage = bootstrap
        .into_source_stage(map_binding(&source), deadline(), okay)
        .unwrap();
    stage
        .publish_source_map(&mut source, &platform, deadline(), okay)
        .unwrap();
    stage.initialize_mapping(deadline(), okay).unwrap();
    for row in rows() {
        stage.push(row, deadline(), okay).unwrap();
    }
    let mut readback = stage.begin_readback(counts(), deadline(), okay).unwrap();
    for row in rows() {
        readback.compare(&row, deadline(), okay).unwrap();
    }
    readback.finish(deadline(), okay).unwrap()
}

#[test]
fn location_v2_adoption_uses_original_publication_profile_in_one_database() {
    let profile = crate::migration::LegacyInventoryProfile::LocationV2;
    let inventory = stream_profile(&rows(), profile);
    let scratch = Scratch::new();
    let mut stage = mapped_profile(&scratch, &inventory, profile)
        .into_inventory_stage(deadline(), okay).unwrap();
    for row in &inventory { stage.push(row, deadline(), okay).unwrap(); }
    let mut reader = stage.begin_readback(deadline(), okay).unwrap();
    for row in &inventory { reader.compare(row, deadline(), okay).unwrap(); }
    let receipt = reader.finish(deadline(), okay).unwrap();
    assert!(receipt.render_json().contains("complete_semantic_candidate_not_performed"));
    for (published, encoded) in [
        (crate::migration::LegacyInventoryProfile::V1, profile),
        (profile, crate::migration::LegacyInventoryProfile::V1),
    ] {
        let scratch = Scratch::new();
        let inventory = stream_profile(&rows(), encoded);
        let mut stage = mapped_profile(&scratch, &inventory, published)
            .into_inventory_stage(deadline(), okay).unwrap();
        assert!(stage.push(&inventory[0], deadline(), okay).is_err());
        assert!(stage.push(&inventory[0], deadline(), okay).is_err());
    }
}

fn readback(scratch: &Scratch, inventory: &[Vec<u8>]) -> OwnerInventoryReadback {
    let mut stage = mapped(scratch, inventory)
        .into_inventory_stage(deadline(), okay)
        .unwrap();
    for row in inventory {
        stage.push(row, deadline(), okay).unwrap();
    }
    stage.begin_readback(deadline(), okay).unwrap()
}

fn content_artifact(
    scratch: &Scratch,
    platform: Platform,
    header: SourceContentManifestHeader,
    objects: &[SourceContentObjectReadback],
) -> (
    Vec<Vec<u8>>,
    crate::migration::SourceImportRecordArtifactPublication<Platform>,
) {
    let mut encoder = SourceContentManifestEncoder::new(header).unwrap();
    let mut rows = vec![encoder.header_row().unwrap()];
    for object in objects { rows.push(encoder.object_row(*object).unwrap()); }
    rows.push(encoder.finish().unwrap().0);
    let mut writer = SourceImportRecordArtifact::create(
        &scratch.0,
        ".source-map.content-stage.tmp",
        platform,
        deadline(),
    )
    .unwrap();
    for row in &rows {
        writer.push(row, deadline()).unwrap();
    }
    let frozen = writer.freeze(deadline()).unwrap();
    let final_name = format!(
        "{}.source-content.v1",
        crate::persistent::bootstrap::codec::hex(frozen.chain())
    );
    let mut comparison = frozen.begin_readback(deadline()).unwrap();
    for row in &rows {
        comparison.compare(row, deadline()).unwrap();
    }
    let publication = comparison
        .finish(deadline())
        .unwrap()
        .prepare_publication(&final_name, deadline())
        .unwrap();
    (rows, publication)
}

#[test]
fn unknown_content_publication_keeps_the_original_attempt_and_blocks_retry() {
    let scratch = Scratch::new();
    let inventory = stream(&rows());
    let mut inventory_readback = readback(&scratch, &inventory);
    for row in &inventory {
        inventory_readback.compare(row, deadline(), okay).unwrap();
    }
    let receipt = inventory_readback.finish(deadline(), okay).unwrap();
    let original = binding();
    let header = SourceContentManifestHeader {
        target_namespace: original.target,
        legacy_namespace: original.legacy_namespace,
        catalog_snapshot: original.catalog_snapshot,
        source_plan: search_contracts::Sha256Digest32::from_bytes(
            *receipt.owner.source.artifact.chain(),
        ),
        expected_objects: 0,
    };
    let platform = Platform::default();
    let (rows, mut publication) = content_artifact(&scratch, platform.clone(), header, &[]);
    let final_name = publication.final_name().to_owned();
    let mut stage = receipt.into_content_stage(0, deadline(), okay).unwrap();

    platform
        .cancel_after_sync
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let failure = stage
        .publish_content(&mut publication, &platform, deadline(), |_| {
            if platform.canceled.load(std::sync::atomic::Ordering::Relaxed) {
                Err("original refused after content link")
            } else {
                Ok(())
            }
        })
        .err()
        .unwrap();
    assert_eq!(failure.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(failure.first_context, Some("original refused after content link"));
    assert!(scratch.0.join(&final_name).is_file());
    assert_eq!(stage.table_count_for_test(), 9);
    assert_eq!(
        stage
            .publish_content(&mut publication, &platform, deadline(), okay)
            .err()
            .unwrap()
            .control,
        ControlError::CommitOutcomeUnknown
    );
    assert_eq!(
        stage.push(&rows[0], deadline(), okay).err().unwrap().control,
        ControlError::CommitOutcomeUnknown
    );
}

#[test]
fn empty_content_stream_is_read_back_as_explicit_missing_occurrences() {
    let scratch = Scratch::new();
    let inventory = stream(&rows());
    let mut inventory_readback = readback(&scratch, &inventory);
    for row in &inventory {
        inventory_readback.compare(row, deadline(), okay).unwrap();
    }
    let receipt = inventory_readback.finish(deadline(), okay).unwrap();
    let expected = counts();
    let original = binding();
    let header = SourceContentManifestHeader {
        target_namespace: original.target,
        legacy_namespace: original.legacy_namespace,
        catalog_snapshot: original.catalog_snapshot,
        source_plan: search_contracts::Sha256Digest32::from_bytes(
            *receipt.owner.source.artifact.chain(),
        ),
        expected_objects: 0,
    };
    let platform = Platform::default();
    let (content_rows, mut publication) =
        content_artifact(&scratch, platform.clone(), header, &[]);
    let mut stage = receipt.into_content_stage(0, deadline(), okay).unwrap();
    stage
        .publish_content(&mut publication, &platform, deadline(), okay)
        .unwrap();
    assert_eq!(stage.table_count_for_test(), 12);
    for row in &content_rows {
        stage.push(row, deadline(), okay).unwrap();
    }
    let mut content_readback = stage.begin_readback(deadline(), okay).unwrap();
    for row in &content_rows {
        content_readback.compare(row, deadline(), okay).unwrap();
    }
    let receipt = content_readback.finish(deadline(), okay).unwrap();
    let json = receipt.render_json();
    assert!(json.contains(&format!(
        "\"occurrences\":{},\"retained_events\":{},\"retirements\":{}",
        expected.occurrences, expected.retained_events, expected.retirements
    )));
    assert!(json.contains(&format!(
        "\"joined_occurrences\":0,\"missing_content_references\":{},\"orphan_content_objects\":0",
        expected.occurrences
    )));
    assert!(json.contains("complete_semantic_candidate_not_performed"));
    assert!(json.contains("\"recovery\":\"unresolved\""));
    assert!(json.contains("\"cleanup\":\"deferred_to_347\""));
    assert!(json.contains("\"cutover\":\"none\""));
}

fn retained_content_object() -> SourceContentObjectReadback {
    let event = rows().remove(0);
    SourceContentObjectReadback {
        legacy_source_id: event.legacy_source,
        legacy_revision_id: event.legacy_revision,
        content_sha256: event.content,
        byte_length: event.source_bytes,
        content_blake3: search_contracts::Blake3Digest32::from_bytes([13; 32]),
    }
}

fn nonempty_content_stage(scratch: &Scratch, object: SourceContentObjectReadback)
    -> (OwnerContentStage, Vec<Vec<u8>>, SourceContentManifestHeader) {
    let inventory = stream(&rows());
    let mut inventory_readback = readback(scratch, &inventory);
    for row in &inventory { inventory_readback.compare(row, deadline(), okay).unwrap(); }
    let receipt = inventory_readback.finish(deadline(), okay).unwrap();
    let original = binding();
    let header = SourceContentManifestHeader {
        target_namespace: original.target,
        legacy_namespace: original.legacy_namespace,
        catalog_snapshot: original.catalog_snapshot,
        source_plan: search_contracts::Sha256Digest32::from_bytes(*receipt.owner.source.artifact.chain()),
        expected_objects: 1,
    };
    let platform = Platform::default();
    let (content_rows, mut publication) = content_artifact(scratch, platform.clone(), header, &[object]);
    let mut stage = receipt.into_content_stage(1, deadline(), okay).unwrap();
    stage.publish_content(&mut publication, &platform, deadline(), okay).unwrap();
    for row in &content_rows { stage.push(row, deadline(), okay).unwrap(); }
    (stage, content_rows, header)
}

#[test]
fn nonempty_content_stream_joins_the_retired_source_in_the_original_database() {
    let scratch = Scratch::new();
    let (stage, content_rows, _) = nonempty_content_stage(&scratch, retained_content_object());
    assert_eq!(stage.table_count_for_test(), 12);
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "redb")).count(), 1);
    let mut reader = stage.begin_readback(deadline(), okay).unwrap();
    for row in &content_rows { reader.compare(row, deadline(), okay).unwrap(); }
    let json = reader.finish(deadline(), okay).unwrap().render_json();
    assert!(json.contains("\"joined_occurrences\":1,\"missing_content_references\":0,\"orphan_content_objects\":0"));
    assert!(json.contains("\"content_objects\":1"));
    assert!(json.contains("complete_semantic_candidate_not_performed"));
}

#[test]
fn nonempty_content_stream_refuses_foreign_sha_length_and_orphan_joins() {
    let correct = retained_content_object();
    let mut wrong_sha = correct;
    wrong_sha.content_sha256 = search_contracts::Sha256Digest32::from_bytes([99; 32]);
    let mut wrong_length = correct;
    wrong_length.byte_length += 1;
    let mut orphan = correct;
    orphan.legacy_revision_id = search_contracts::Sha256Digest32::from_bytes([99; 32]);
    for object in [wrong_sha, wrong_length, orphan] {
        let scratch = Scratch::new();
        let (stage, content_rows, _) = nonempty_content_stage(&scratch, object);
        let mut reader = stage.begin_readback(deadline(), okay).unwrap();
        for row in &content_rows { reader.compare(row, deadline(), okay).unwrap(); }
        assert_eq!(reader.finish(deadline(), okay).err().unwrap().control, ControlError::MigrationUnverified);
    }
}

#[test]
fn nonempty_content_readback_mismatch_latches_and_cannot_finish() {
    let scratch = Scratch::new();
    let (stage, content_rows, header) = nonempty_content_stage(&scratch, retained_content_object());
    let mut reader = stage.begin_readback(deadline(), okay).unwrap();
    reader.compare(&content_rows[0], deadline(), okay).unwrap();
    let mut different = retained_content_object();
    different.content_blake3 = search_contracts::Blake3Digest32::from_bytes([99; 32]);
    let mut encoder = SourceContentManifestEncoder::new(header).unwrap();
    encoder.header_row().unwrap();
    let changed = encoder.object_row(different).unwrap();
    let first = reader.compare(&changed, deadline(), okay).err().unwrap().control;
    assert_eq!(reader.compare(&content_rows[1], deadline(), okay).err().unwrap().control, first);
    assert_eq!(reader.finish(deadline(), okay).err().unwrap().control, first);
}

#[test]
fn complete_inventory_rows_and_all_five_source_counts_share_one_inactive_database() {
    let scratch = Scratch::new();
    let inventory = stream(&rows());
    let mut reader = readback(&scratch, &inventory);
    let transaction = reader
        .owner
        .source
        .owner
        .bootstrap
        .journal
        .database
        .begin_read()
        .unwrap();
    assert_eq!(transaction.list_tables().unwrap().count(), 9);
    assert!(
        crate::persistent::verify_tables(
            &transaction,
            &crate::persistent::TableProfile::ControlOnly
        )
        .is_err()
    );
    drop(transaction);
    for row in &inventory {
        reader.compare(row, deadline(), okay).unwrap();
    }
    let receipt = reader.finish(deadline(), okay).unwrap();
    let json = receipt.render_json();
    assert!(json.contains("\"retained_events\":1,\"retirements\":1"));
    assert_eq!(json.matches("\"family\":").count(), 12);
    assert!(json.contains("complete_semantic_candidate_not_performed"));
    assert_eq!(
        std::fs::read_dir(&scratch.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "redb"))
            .count(),
        1
    );
}

#[test]
fn a_canonical_stream_different_from_original_published_inventory_cannot_seal() {
    let scratch = Scratch::new();
    let inventory = stream(&rows());
    let mut stage = mapped(&scratch, &inventory)
        .into_inventory_stage(deadline(), okay)
        .unwrap();
    let mut changed = rows();
    changed[0].path = search_contracts::Sha256Digest32::from_bytes([99; 32]);
    for row in stream(&changed) {
        stage.push(&row, deadline(), okay).unwrap();
    }
    assert_eq!(
        stage
            .begin_readback(deadline(), okay)
            .err()
            .unwrap()
            .control,
        ControlError::MigrationUnverified
    );
}

#[test]
fn inventory_readback_original_refusal_is_sticky_and_never_yields_receipt() {
    let scratch = Scratch::new();
    let inventory = stream(&rows());
    let mut reader = readback(&scratch, &inventory);
    let error = reader
        .compare(&inventory[0], deadline(), |_| Err("original refusal"))
        .err()
        .unwrap();
    assert_eq!(error.first_context, Some("original refusal"));
    assert_eq!(
        reader
            .compare(&inventory[0], deadline(), okay)
            .err()
            .unwrap()
            .control,
        error.control
    );
    assert_eq!(
        reader.finish(deadline(), okay).err().unwrap().control,
        error.control
    );
}

#[test]
fn duplicate_terminal_after_all_twelve_families_refuses_without_panicking() {
    let inventory = stream(&rows());
    let observed = binding();
    let mut decoder = LegacyInventoryDecoder::new(
        LegacyInventoryBinding {
            owner: observed.owner,
            target: observed.target,
            namespace: observed.legacy_namespace,
            snapshot: observed.catalog_snapshot,
            registration: observed.registration,
        },
        LegacyInventoryLimits::BASELINE,
    )
    .unwrap();
    for row in &inventory[..inventory.len() - 1] {
        decoder.push(row).unwrap();
    }
    assert_eq!(
        decoder.push(&inventory[inventory.len() - 2]).err().unwrap(),
        LegacyInventoryError::Accounting
    );
    assert_eq!(
        decoder.finish().err().unwrap(),
        LegacyInventoryError::Accounting
    );
}
