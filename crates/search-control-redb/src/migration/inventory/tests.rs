use search_contracts::{Sha256Digest32, SourceId, SourceNamespaceId, SourceRevisionId};

use super::*;
use crate::migration::{SourceImportRow, SourceLifecycleFlags};

fn digest(byte: u8) -> Sha256Digest32 {
    Sha256Digest32::from_bytes([byte; 32])
}

fn repeated_hex(byte: u8, count: usize) -> String {
    format!("{byte:02x}").repeat(count)
}

fn binding() -> LegacyInventoryBinding {
    LegacyInventoryBinding {
        owner: LegacyInventoryOwner {
            installation: [1; 16],
            incarnation: [2; 16],
            root: [3; 16],
            invocation: [4; 16],
            request_blake3: [5; 32],
            record_blake3: [6; 32],
            physical_blake3: [7; 32],
            native_limits: [256, 262_144, 524_288, 1_073_741_824, 16_384, 256],
            epoch: 8,
            generation: 9,
        },
        target: SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
            .expect("valid target namespace"),
        namespace: digest(11),
        snapshot: digest(12),
        registration: Some((digest(13), 14)),
    }
}

#[test]
fn location_v2_is_explicit_and_old_decoder_keeps_frozen_v1_semantics() {
    let limits = LegacyInventoryLimits::BASELINE;
    let mut old = LegacyInventoryEncoder::new(limits).unwrap();
    assert_eq!(old.header(binding()).unwrap(), expected_header().into_bytes());
    assert_eq!(LegacyInventoryProfile::V1.digest(), legacy_inventory_profile_digest());
    assert_ne!(LegacyInventoryProfile::LocationV2.digest(), LegacyInventoryProfile::V1.digest());
    for selected in [LegacyInventoryProfile::V1, LegacyInventoryProfile::LocationV2] {
        let mut encoder = LegacyInventoryEncoder::with_profile(limits, selected).unwrap();
        let mut decoder = LegacyInventoryDecoder::with_profile(binding(), limits, selected).unwrap();
        let header = encoder.header(binding()).unwrap();
        assert_eq!(decoder.push(&header).unwrap(), LegacyInventoryDecodedRow::Header);
        let other = if selected == LegacyInventoryProfile::V1 {
            LegacyInventoryProfile::LocationV2
        } else { LegacyInventoryProfile::V1 };
        let mut mismatch = LegacyInventoryDecoder::with_profile(binding(), limits, other).unwrap();
        assert!(mismatch.push(&header).is_err());
        assert!(mismatch.push(&header).is_err());
        for family in LegacyInventoryFamily::ALL {
            let row = encoder.end_family(family, LegacyInventoryStatus::AbsentProven, true).unwrap();
            decoder.push(&row).unwrap();
        }
        let (footer, summary) = encoder.finish().unwrap();
        assert_eq!(decoder.push(&footer).unwrap(), LegacyInventoryDecodedRow::Footer(summary));
        assert_eq!(decoder.finish().unwrap(), summary);
    }
}

fn source_row() -> SourceImportRow {
    SourceImportRow {
        sequence: 1,
        source: SourceId::from_bytes([1; 16]),
        revision: SourceRevisionId::from_bytes([2; 16]),
        previous_revision: None,
        occurrence: 1,
        source_event: 1,
        source_bytes: 3,
        lifecycle: SourceLifecycleFlags::new([true, true, false, true]),
        operation: digest(4),
        legacy_source: digest(5),
        legacy_revision: digest(6),
        content: digest(7),
        file_identity: digest(8),
        path: digest(9),
        event: digest(10),
        previous_event: digest(11),
        previous_source_event: digest(12),
    }
}

fn expected_header() -> String {
    format!(
        "H\teliot.legacy-inventory.v1\t{}\t{}\t{}\t{}\tblake3:{}\tblake3:{}\tblake3:{}\t8\t9\t123e4567-e89b-12d3-a456-426614174000\tsha256:{}\tsha256:{}\tsha256:{}:14\t2000000\t536870912\t32\tnative-limits=256,262144,524288,1073741824,16384,256\tauthority=none\n",
        repeated_hex(1, 16),
        repeated_hex(2, 16),
        repeated_hex(3, 16),
        repeated_hex(4, 16),
        repeated_hex(5, 32),
        repeated_hex(6, 32),
        repeated_hex(7, 32),
        repeated_hex(11, 32),
        repeated_hex(12, 32),
        repeated_hex(13, 32),
    )
}

fn expected_source_event() -> String {
    format!(
        "E\tsources\tpresent\t{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}\n",
        repeated_hex(1, 16),
        repeated_hex(2, 16),
        "00",
        repeated_hex(0, 16),
        format!("{:016x}", 1),
        format!("{:016x}", 1),
        format!("{:016x}", 1),
        format!("{:016x}", 3),
        "0301",
        repeated_hex(4, 32),
        repeated_hex(5, 32),
        repeated_hex(6, 32),
        repeated_hex(7, 32),
        repeated_hex(8, 32),
        repeated_hex(9, 32),
        repeated_hex(10, 32),
        repeated_hex(11, 32),
        repeated_hex(12, 32),
    )
}

fn item(
    encoder: &mut LegacyInventoryEncoder,
    family: LegacyInventoryFamily,
    status: LegacyInventoryStatus,
) -> Result<Vec<u8>, LegacyInventoryError> {
    encoder.item(
        family,
        status,
        7,
        [digest(0xa1), digest(0xb2), digest(0xc3)],
        [4, 5, 6],
    )
}

fn finish_absent_families(encoder: &mut LegacyInventoryEncoder) {
    for family in LegacyInventoryFamily::ALL.into_iter().skip(1) {
        encoder
            .end_family(family, LegacyInventoryStatus::AbsentProven, true)
            .expect("empty family has a complete empty denominator");
    }
}

#[test]
fn profile_golden_covers_header_source_event_all_statuses_terminals_and_footer() {
    let statuses = [
        LegacyInventoryStatus::Present,
        LegacyInventoryStatus::AbsentProven,
        LegacyInventoryStatus::Unsupported,
        LegacyInventoryStatus::Corrupt,
        LegacyInventoryStatus::Unreadable,
        LegacyInventoryStatus::Orphan,
        LegacyInventoryStatus::Gap,
    ];
    assert_eq!(
        statuses.map(LegacyInventoryStatus::tag),
        [
            "present",
            "absent-proven",
            "unsupported",
            "corrupt",
            "unreadable",
            "orphan",
            "gap",
        ]
    );

    let mut encoder =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    let mut actual_rows = vec![encoder.header(binding()).expect("header")];
    let mut expected_rows = vec![expected_header().into_bytes()];
    for status in statuses {
        actual_rows.push(item(&mut encoder, LegacyInventoryFamily::Owner, status).expect("item"));
        expected_rows.push(
            format!(
                "I\towner\t{}\t7\t{}\t{}\t{}\t4\t5\t6\n",
                status.tag(),
                repeated_hex(0xa1, 32),
                repeated_hex(0xb2, 32),
                repeated_hex(0xc3, 32),
            )
            .into_bytes(),
        );
    }
    actual_rows.push(
        encoder
            .end_family(
                LegacyInventoryFamily::Owner,
                LegacyInventoryStatus::Gap,
                false,
            )
            .expect("gap dominates the seven dispositions"),
    );
    expected_rows.push(b"T\towner\tgap\t0\t1,1,1,1,1,1,1\n".to_vec());

    actual_rows.push(encoder.source_event(&source_row()).expect("source event"));
    expected_rows.push(expected_source_event().into_bytes());
    actual_rows.push(
        encoder
            .end_family(
                LegacyInventoryFamily::Sources,
                LegacyInventoryStatus::Present,
                true,
            )
            .expect("source events are present and fully accounted"),
    );
    expected_rows.push(b"T\tsources\tpresent\t1\t1,0,0,0,0,0,0\n".to_vec());

    let empty_terminal = "\tabsent-proven\t1\t0,0,0,0,0,0,0\n";
    for (family, tag) in LegacyInventoryFamily::ALL.into_iter().skip(2).zip([
        "roots",
        "directory-manifests",
        "revisions",
        "preparation",
        "residue",
        "operations",
        "publications",
        "security",
        "credentials",
        "limits",
    ]) {
        actual_rows.push(
            encoder
                .end_family(family, LegacyInventoryStatus::AbsentProven, true)
                .expect("empty family terminal"),
        );
        expected_rows.push(format!("T\t{tag}{empty_terminal}").into_bytes());
    }

    assert_eq!(actual_rows, expected_rows);
    let prefix_bytes = expected_rows.iter().map(Vec::len).sum::<usize>();
    assert_eq!(expected_rows.len(), 21);
    let expected_footer = format!(
        "F\teliot.legacy-inventory.v1\t12\t21\t{prefix_bytes}\trecovery=unresolved\tcutover=none\n"
    );
    let (footer, summary) = encoder.finish().expect("complete inventory footer");
    assert_eq!(footer, expected_footer.as_bytes());
    assert_eq!(summary.records, 22);
    assert_eq!(
        summary.encoded_bytes,
        prefix_bytes as u64 + footer.len() as u64
    );
    assert_eq!(
        summary.counts[LegacyInventoryFamily::Owner as usize],
        [1; 7]
    );
    assert_eq!(
        summary.counts[LegacyInventoryFamily::Sources as usize],
        [1, 0, 0, 0, 0, 0, 0]
    );
    assert!(!summary.denominators[LegacyInventoryFamily::Owner as usize]);
    assert!(
        summary.denominators[LegacyInventoryFamily::Sources as usize..]
            .iter()
            .all(|complete| *complete)
    );
    assert!(!String::from_utf8_lossy(&footer).contains("C:\\"));
}

#[test]
fn all_terminal_dispositions_match_their_counts_and_completeness() {
    let cases = [
        (LegacyInventoryStatus::Present, true),
        (LegacyInventoryStatus::AbsentProven, true),
        (LegacyInventoryStatus::Unsupported, false),
        (LegacyInventoryStatus::Corrupt, true),
        (LegacyInventoryStatus::Unreadable, false),
        (LegacyInventoryStatus::Orphan, true),
        (LegacyInventoryStatus::Gap, false),
    ];
    for (terminal, complete) in cases {
        let mut encoder =
            LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
        encoder.header(binding()).expect("header");
        item(&mut encoder, LegacyInventoryFamily::Owner, terminal).expect("disposition item");
        encoder
            .end_family(LegacyInventoryFamily::Owner, terminal, complete)
            .expect("matching dominant terminal");
        finish_absent_families(&mut encoder);
        encoder.finish().expect("complete artifact");
    }
}

#[test]
fn record_cap_rejection_is_atomic_and_poisoned() {
    let limits = LegacyInventoryLimits {
        records: 14,
        ..LegacyInventoryLimits::BASELINE
    };
    let mut encoder = LegacyInventoryEncoder::new(limits).expect("minimum valid record limit");
    encoder.header(binding()).expect("header");
    for family in LegacyInventoryFamily::ALL.into_iter().take(6) {
        item(&mut encoder, family, LegacyInventoryStatus::Present).expect("family item");
        encoder
            .end_family(family, LegacyInventoryStatus::Present, true)
            .expect("complete family");
    }
    item(
        &mut encoder,
        LegacyInventoryFamily::Residue,
        LegacyInventoryStatus::Present,
    )
    .expect("fourteenth record");
    let before = encoder.summary;
    assert_eq!(
        item(
            &mut encoder,
            LegacyInventoryFamily::Residue,
            LegacyInventoryStatus::Present
        ),
        Err(LegacyInventoryError::Limit)
    );
    assert_eq!(encoder.summary, before, "rejected row changed accounting");
    assert!(encoder.poisoned);
    assert_eq!(encoder.finish(), Err(LegacyInventoryError::Accounting));
}

#[test]
fn row_and_byte_caps_are_finite_and_never_truncate() {
    assert_eq!(LegacyInventoryLimits::BASELINE.records, 2_000_000);
    assert_eq!(
        LegacyInventoryLimits::BASELINE.encoded_bytes,
        512 * 1024 * 1024
    );
    assert_eq!(LegacyInventoryLimits::BASELINE.page_names, 32);
    assert_eq!(super::super::MAX_SOURCE_IMPORT_ROW_BYTES, 8_192);
    assert!(
        LegacyInventoryLimits {
            records: 2_000_001,
            ..LegacyInventoryLimits::BASELINE
        }
        .validate()
        .is_err()
    );
    assert!(
        LegacyInventoryLimits {
            encoded_bytes: 512 * 1024 * 1024 + 1,
            ..LegacyInventoryLimits::BASELINE
        }
        .validate()
        .is_err()
    );
    assert!(
        LegacyInventoryLimits {
            page_names: 33,
            ..LegacyInventoryLimits::BASELINE
        }
        .validate()
        .is_err()
    );

    let mut exact_row =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    let full = vec![b'x'; super::super::MAX_SOURCE_IMPORT_ROW_BYTES];
    let encoded = exact_row
        .row(String::from_utf8(full.clone()).expect("ASCII row"))
        .expect("8192-byte row is retained exactly");
    assert_eq!(encoded, full);
    assert_eq!(exact_row.summary.encoded_bytes, 8_192);

    let mut over_row =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    let before = over_row.summary;
    assert_eq!(
        over_row.row("x".repeat(8_193)),
        Err(LegacyInventoryError::Limit)
    );
    assert_eq!(over_row.summary, before);
    assert!(over_row.poisoned);
    assert_eq!(over_row.finish(), Err(LegacyInventoryError::Accounting));

    let byte_limited = LegacyInventoryLimits {
        encoded_bytes: expected_header().len() as u64 + 32,
        ..LegacyInventoryLimits::BASELINE
    };
    let mut encoder = LegacyInventoryEncoder::new(byte_limited).expect("header-sized byte cap");
    let header = encoder
        .header(binding())
        .expect("header fits inside byte cap");
    assert!(header.len() < byte_limited.encoded_bytes as usize);
    let before = encoder.summary;
    assert_eq!(
        item(
            &mut encoder,
            LegacyInventoryFamily::Owner,
            LegacyInventoryStatus::Present
        ),
        Err(LegacyInventoryError::Limit)
    );
    assert_eq!(encoder.summary, before, "byte rejection changed accounting");
    assert_eq!(encoder.finish(), Err(LegacyInventoryError::Accounting));
}

#[test]
fn actual_native_allowance_is_distinct_and_a_missing_limit_poisoned_the_header() {
    let mut first = LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).unwrap();
    let baseline = first.header(binding()).unwrap();
    let mut changed = binding();
    changed.owner.native_limits[3] -= 1;
    let mut second = LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).unwrap();
    assert_ne!(baseline, second.header(changed).unwrap());

    let mut absent = binding();
    absent.owner.native_limits[4] = 0;
    let mut refused = LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).unwrap();
    assert_eq!(refused.header(absent), Err(LegacyInventoryError::Accounting));
    assert_eq!(refused.summary, LegacyInventorySummary::default());
    assert_eq!(refused.finish(), Err(LegacyInventoryError::Accounting));
}

#[test]
fn wrong_order_missing_terminal_and_repeated_header_fail_closed() {
    let mut wrong_order =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    wrong_order.header(binding()).expect("header");
    let before = wrong_order.summary;
    assert_eq!(
        item(
            &mut wrong_order,
            LegacyInventoryFamily::Sources,
            LegacyInventoryStatus::Present
        ),
        Err(LegacyInventoryError::Accounting)
    );
    assert_eq!(wrong_order.summary, before);
    assert!(wrong_order.poisoned);
    assert_eq!(wrong_order.finish(), Err(LegacyInventoryError::Accounting));

    let mut missing_terminal =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    missing_terminal.header(binding()).expect("header");
    assert_eq!(
        missing_terminal.finish(),
        Err(LegacyInventoryError::Accounting)
    );

    let mut repeated_header =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    repeated_header.header(binding()).expect("first header");
    let before = repeated_header.summary;
    assert_eq!(
        repeated_header.header(binding()),
        Err(LegacyInventoryError::Accounting)
    );
    assert_eq!(repeated_header.summary, before);
    assert!(repeated_header.poisoned);
    assert_eq!(
        repeated_header.finish(),
        Err(LegacyInventoryError::Accounting)
    );
}

#[test]
fn terminal_absence_and_incomplete_denominators_cannot_be_forged() {
    let mut false_absence =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    false_absence.header(binding()).expect("header");
    item(
        &mut false_absence,
        LegacyInventoryFamily::Owner,
        LegacyInventoryStatus::Present,
    )
    .expect("present evidence");
    let before = false_absence.summary;
    assert_eq!(
        false_absence.end_family(
            LegacyInventoryFamily::Owner,
            LegacyInventoryStatus::AbsentProven,
            true,
        ),
        Err(LegacyInventoryError::Accounting)
    );
    assert_eq!(false_absence.summary, before);
    assert_eq!(
        false_absence.finish(),
        Err(LegacyInventoryError::Accounting)
    );

    for status in [
        LegacyInventoryStatus::Unsupported,
        LegacyInventoryStatus::Unreadable,
        LegacyInventoryStatus::Gap,
    ] {
        let mut incomplete =
            LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
        incomplete.header(binding()).expect("header");
        item(&mut incomplete, LegacyInventoryFamily::Owner, status).expect("status item");
        let before = incomplete.summary;
        assert_eq!(
            incomplete.end_family(LegacyInventoryFamily::Owner, status, true),
            Err(LegacyInventoryError::Accounting),
            "{status:?} cannot claim a complete denominator"
        );
        assert_eq!(incomplete.summary, before);
        assert_eq!(incomplete.finish(), Err(LegacyInventoryError::Accounting));
    }

    let mut wrong_dominance =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    wrong_dominance.header(binding()).expect("header");
    item(
        &mut wrong_dominance,
        LegacyInventoryFamily::Owner,
        LegacyInventoryStatus::Gap,
    )
    .expect("gap item");
    assert_eq!(
        wrong_dominance.end_family(
            LegacyInventoryFamily::Owner,
            LegacyInventoryStatus::Present,
            true,
        ),
        Err(LegacyInventoryError::Accounting)
    );
    assert_eq!(
        wrong_dominance.finish(),
        Err(LegacyInventoryError::Accounting)
    );

    let mut mixed_incomplete =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    mixed_incomplete.header(binding()).expect("header");
    item(
        &mut mixed_incomplete,
        LegacyInventoryFamily::Owner,
        LegacyInventoryStatus::Corrupt,
    )
    .expect("corrupt item");
    item(
        &mut mixed_incomplete,
        LegacyInventoryFamily::Owner,
        LegacyInventoryStatus::Unsupported,
    )
    .expect("unsupported item");
    assert_eq!(
        mixed_incomplete.end_family(
            LegacyInventoryFamily::Owner,
            LegacyInventoryStatus::Corrupt,
            true,
        ),
        Err(LegacyInventoryError::Accounting),
        "unsupported evidence prevents a complete denominator and dominates corrupt"
    );
}

#[test]
fn rejected_source_metadata_poisoned_the_stream_without_accounting_it() {
    let mut encoder =
        LegacyInventoryEncoder::new(LegacyInventoryLimits::BASELINE).expect("baseline limits");
    encoder.header(binding()).expect("header");
    let mut invalid = source_row();
    invalid.sequence = 0;
    let before = encoder.summary;
    assert_eq!(
        encoder.source_event(&invalid),
        Err(LegacyInventoryError::Metadata)
    );
    assert_eq!(encoder.summary, before);
    assert!(encoder.poisoned);
    assert_eq!(encoder.finish(), Err(LegacyInventoryError::Accounting));
}
