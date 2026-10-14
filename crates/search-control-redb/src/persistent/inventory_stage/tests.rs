//! Same-owner component fixtures with injected native identities, not product qualification.
use super::super::super::tests::{Platform, Scratch, binding, deadline, fixture_digest};
use super::super::tests::{counts, map_binding, rows, source_map};
use super::*;
use crate::MigrationBootstrap;
use crate::migration::{
    LegacyInventoryEncoder, LegacyInventoryFamily as Family, LegacyInventoryStatus as Status,
    SourceImportRecordArtifact, SourceImportRow,
};

fn okay(_: Work) -> Result<(), ()> {
    Ok(())
}

fn stream(events: &[SourceImportRow]) -> Vec<Vec<u8>> {
    let observed = binding();
    let mut encoder = LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).unwrap();
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
        "{}.legacy-inventory.v1",
        crate::persistent::bootstrap::codec::hex(frozen.chain())
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

fn readback(scratch: &Scratch, inventory: &[Vec<u8>]) -> OwnerInventoryReadback {
    let mut stage = mapped(scratch, inventory)
        .into_inventory_stage(deadline(), okay)
        .unwrap();
    for row in inventory {
        stage.push(row, deadline(), okay).unwrap();
    }
    stage.begin_readback(deadline(), okay).unwrap()
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
