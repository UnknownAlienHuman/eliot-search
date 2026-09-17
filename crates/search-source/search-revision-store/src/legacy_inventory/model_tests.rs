use super::*;

struct TestDigest;

impl LegacyRevisionInventoryDigest for TestDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        fold(domain, parts)
    }
}

fn fold(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut output = [0_u8; 32];
    let mut index = 0_usize;
    for byte in domain
        .iter()
        .chain(parts.iter().flat_map(|part| part.iter()))
    {
        let slot = index % output.len();
        output[slot] = output[slot]
            .wrapping_add(*byte)
            .rotate_left(u32::try_from(index % 8).unwrap_or(0));
        index += 1;
    }
    output
}

fn id(pair: &str) -> String {
    pair.repeat(32)
}

fn final_entry(
    pair: &str,
    suffix: &str,
    encoded_bytes: u64,
    modified_seconds: u64,
    modified_nanos: u32,
    kind: LegacyRevisionInventoryKind,
) -> LegacyRevisionInventoryEntry {
    LegacyRevisionInventoryEntry::new(
        format!("{pair}/{}.{suffix}", id(pair)),
        encoded_bytes,
        modified_seconds,
        modified_nanos,
        kind,
    )
    .expect("final entry")
}

fn temporary_entry(
    pair: &str,
    encoded_bytes: u64,
) -> LegacyRevisionInventoryEntry {
    LegacyRevisionInventoryEntry::new(
        format!("{pair}/.{}.7.tmp", id(pair)),
        encoded_bytes,
        5,
        6,
        LegacyRevisionInventoryKind::Temporary,
    )
    .expect("temporary entry")
}

#[test]
fn inventory_sorting_counts_and_digest_inputs_are_deterministic() {
    let inventory = build_legacy_revision_inventory::<TestDigest>(
        vec!["bb".to_owned(), "aa".to_owned()],
        vec![
            final_entry(
                "bb",
                "dpapi",
                20,
                3,
                4,
                LegacyRevisionInventoryKind::Orphan,
            ),
            final_entry(
                "aa",
                "bin",
                10,
                1,
                2,
                LegacyRevisionInventoryKind::Referenced,
            ),
            temporary_entry("aa", 30),
        ],
    )
    .expect("inventory");

    assert_eq!(inventory.shards(), &["aa".to_owned(), "bb".to_owned()]);
    assert_eq!(inventory.object_count(), 3);
    assert_eq!(inventory.referenced_count(), 1);
    assert_eq!(inventory.orphan_count(), 1);
    assert_eq!(inventory.temporary_count(), 1);
    assert_eq!(inventory.unreferenced_bytes(), 50);
    let locators = inventory
        .entries()
        .iter()
        .map(LegacyRevisionInventoryEntry::relative_locator)
        .collect::<Vec<_>>();
    let expected_locators = [
        format!("aa/.{}.7.tmp", id("aa")),
        format!("aa/{}.bin", id("aa")),
        format!("bb/{}.dpapi", id("bb")),
    ];
    assert_eq!(
        locators,
        expected_locators.iter().map(String::as_str).collect::<Vec<_>>()
    );

    let reversed = build_legacy_revision_inventory::<TestDigest>(
        vec!["aa".to_owned(), "bb".to_owned()],
        inventory.entries().iter().cloned().rev().collect(),
    )
    .expect("reversed inventory");
    assert_eq!(inventory, reversed);
}

#[test]
fn model_rejects_missing_shards_duplicates_and_inconsistent_kinds() {
    let orphan = final_entry(
        "aa",
        "bin",
        1,
        0,
        0,
        LegacyRevisionInventoryKind::Orphan,
    );
    assert_eq!(
        build_legacy_revision_inventory::<TestDigest>(vec![], vec![orphan.clone()]),
        Err(LegacyRevisionInventoryError::UnexpectedObject)
    );
    assert_eq!(
        build_legacy_revision_inventory::<TestDigest>(
            vec!["aa".to_owned(), "aa".to_owned()],
            vec![],
        ),
        Err(LegacyRevisionInventoryError::UnexpectedObject)
    );
    assert_eq!(
        build_legacy_revision_inventory::<TestDigest>(
            vec!["aa".to_owned()],
            vec![orphan.clone(), orphan],
        ),
        Err(LegacyRevisionInventoryError::UnexpectedObject)
    );
    assert_eq!(
        LegacyRevisionInventoryEntry::new(
            format!("aa/{}.bin", id("aa")),
            1,
            0,
            0,
            LegacyRevisionInventoryKind::Temporary,
        ),
        Err(LegacyRevisionInventoryError::UnexpectedObject)
    );
    assert_eq!(
        LegacyRevisionInventoryEntry::new(
            format!("aa/.{}.7.tmp", id("aa")),
            1,
            0,
            1_000_000_000,
            LegacyRevisionInventoryKind::Temporary,
        ),
        Err(LegacyRevisionInventoryError::UnexpectedObject)
    );
}

#[test]
fn cursor_and_page_selection_are_canonical_bounded_and_stale_safe() {
    let mut entries = vec![final_entry(
        "00",
        "bin",
        1,
        0,
        0,
        LegacyRevisionInventoryKind::Referenced,
    )];
    for value in 1_u8..=8 {
        let pair = format!("{value:02x}");
        entries.push(final_entry(
            &pair,
            "dpapi",
            u64::try_from(LEGACY_REVISION_MAX_OBJECT_BYTES).unwrap_or(u64::MAX),
            u64::from(value),
            0,
            LegacyRevisionInventoryKind::Orphan,
        ));
    }
    let shards = (0_u8..=8).map(|value| format!("{value:02x}")).collect();
    let inventory =
        build_legacy_revision_inventory::<TestDigest>(shards, entries).expect("inventory");
    let catalog = [0x33; 32];
    let checkpoint = legacy_revision_inventory_checkpoint::<TestDigest>(
        &catalog,
        "dpapi",
        &inventory,
    );
    let first = plan_legacy_revision_inventory_page(&inventory, checkpoint, None)
        .expect("first page");
    assert_eq!(first.first(), 0);
    assert_eq!(first.object_count(), 7);
    assert_eq!(first.next(), 7);
    assert!(!first.exhausted());

    let encoded = first.next_cursor().expect("next cursor");
    let cursor = LegacyRevisionInventoryCursor::parse(&encoded).expect("cursor");
    assert_eq!(cursor.encode(), encoded);
    let second = plan_legacy_revision_inventory_page(
        &inventory,
        checkpoint,
        Some(&cursor),
    )
    .expect("second page");
    assert_eq!(second.first(), 7);
    assert_eq!(second.object_count(), 1);
    assert!(second.exhausted());
    assert_eq!(second.next_cursor(), None);

    assert_eq!(
        LegacyRevisionInventoryCursor::parse("o1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.0"),
        Err(LegacyRevisionInventoryError::CursorInvalid)
    );
    assert_eq!(
        LegacyRevisionInventoryCursor::parse(&format!(
            "o1.{}.07",
            "00".repeat(32)
        )),
        Err(LegacyRevisionInventoryError::CursorInvalid)
    );
    assert_eq!(
        plan_legacy_revision_inventory_page(
            &inventory,
            [0x44; 32],
            Some(&cursor),
        ),
        Err(LegacyRevisionInventoryError::CursorStale)
    );
}

#[test]
fn canonical_report_bytes_and_digest_domains_are_frozen() {
    let entry = final_entry(
        "ab",
        "bin",
        4,
        1,
        2,
        LegacyRevisionInventoryKind::Orphan,
    );
    let inventory = build_legacy_revision_inventory::<TestDigest>(
        vec!["ab".to_owned()],
        vec![entry],
    )
    .expect("inventory");
    assert_eq!(
        super::wire::hex(inventory.digest()),
        "8a12316e584721679666a3fa4f9dc67ebd5b0c21d7f97da3edb2f8451916cade"
    );
    let catalog = [0x11; 32];
    let checkpoint = legacy_revision_inventory_checkpoint::<TestDigest>(
        &catalog,
        "dpapi",
        &inventory,
    );
    assert_eq!(
        super::wire::hex(&checkpoint),
        "f4c8b3c534a478229bc713714d3c711bb6b24e57f5c71c6426436bfb632b35df"
    );
    let page = plan_legacy_revision_inventory_page(&inventory, checkpoint, None)
        .expect("page");
    let evidence = vec![LegacyRevisionObjectEvidence::new(
        &page.entries()[0],
        [0x22; 32],
    )];
    let report = render_legacy_revision_inventory_report::<TestDigest>(
        "namespace",
        &catalog,
        &inventory,
        &page,
        &evidence,
    )
    .expect("report");
    assert_eq!(
        report,
        concat!(
            "{\"event\":\"control_migration_orphans\",",
            "\"schema\":\"legacy-revision-orphans-v1\",",
            "\"scope\":\"revision_tree_only\",",
            "\"namespace_id\":\"namespace\",",
            "\"catalog_snapshot_sha256\":\"1111111111111111111111111111111111111111111111111111111111111111\",",
            "\"inventory_sha256\":\"8a12316e584721679666a3fa4f9dc67ebd5b0c21d7f97da3edb2f8451916cade\",",
            "\"inventory_basis\":\"names_sizes_mtimes\",",
            "\"shards\":1,\"inventory_files\":1,",
            "\"catalog_referenced_files\":0,",
            "\"orphan_objects\":1,\"temporary_objects\":0,",
            "\"unreferenced_bytes\":4,",
            "\"after_object\":0,\"next_object\":1,",
            "\"page_objects\":1,\"page_encoded_bytes\":4,",
            "\"entries\":[{\"object_locator\":\"revisions/ab/abababababababababababababababababababababababababababababababab.bin\",",
            "\"kind\":\"unreferenced_revision_object\",",
            "\"encoded_bytes\":4,",
            "\"encoded_sha256\":\"2222222222222222222222222222222222222222222222222222222222222222\",",
            "\"catalog_referenced\":false,",
            "\"source_binding_verified\":false,",
            "\"deletion_authorized\":false}],",
            "\"page_sha256\":\"19fede021cb630e99b5ef64e7032c816cdd7d7231aafea477200ab629920adfc\",",
            "\"next_cursor\":null,\"exhausted\":true,",
            "\"read_only\":true,",
            "\"page_encoded_bytes_hashed\":true,",
            "\"all_object_contents_hashed\":false,",
            "\"preparation_orphans_enumerated\":false,",
            "\"deletion_authorized\":false,",
            "\"cutover_revalidation_required\":true,",
            "\"canonical_mapping_complete\":false}"
        )
    );
}
