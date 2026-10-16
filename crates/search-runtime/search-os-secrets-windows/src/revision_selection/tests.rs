//! Pure refusals only: public metadata decode never creates a live dependency.

use super::*;
use std::time::Instant;

// Construct untrusted fixed-profile CBOR for the public Core decoders. This
// fixture needs no new direct contracts/config dependency or native setup.
fn byte_string(output: &mut Vec<u8>, value: &[u8]) {
    let length = u16::try_from(value.len()).unwrap();
    if length < 24 {
        output.push(0x40 | u8::try_from(length).unwrap());
    } else if let Ok(short) = u8::try_from(length) {
        output.extend_from_slice(&[0x58, short]);
    } else {
        output.push(0x59);
        output.extend_from_slice(&length.to_be_bytes());
    }
    output.extend_from_slice(value);
}

fn text(output: &mut Vec<u8>, value: &str) {
    let length = u8::try_from(value.len()).unwrap();
    if length < 24 {
        output.push(0x60 | length);
    } else {
        output.extend_from_slice(&[0x78, length]);
    }
    output.extend_from_slice(value.as_bytes());
}

fn intent(config: u8, candidate_epoch: u8) -> RevisionDekApplicationIntent {
    let binding = core_secrets::RevisionDekBinding::new(
        "01010101-0101-0101-0101-010101010101".parse().unwrap(),
        "02020202-0202-0202-0202-020202020202".parse().unwrap(),
        "03030303-0303-0303-0303-030303030303".parse().unwrap(),
        [4; 32],
        [5; 16],
    )
    .unwrap();
    let mut candidate = vec![0x87];
    text(&mut candidate, "eliot.revision-dek.record.v1");
    candidate.extend_from_slice(&[1, 1, 0x8b]);
    for id in [&[1; 16], &[2; 16], &[3; 16]] {
        byte_string(&mut candidate, id);
    }
    byte_string(&mut candidate, &[4; 32]);
    byte_string(&mut candidate, &[5; 16]);
    byte_string(&mut candidate, &binding.reference_digest());
    candidate.extend_from_slice(&[1, 1]);
    text(&mut candidate, core_secrets::REVISION_DEK_PURPOSE);
    text(&mut candidate, core_secrets::REVISION_DEK_BACKEND);
    candidate.extend_from_slice(&[1, candidate_epoch]);
    byte_string(&mut candidate, &[9; 32]);
    candidate.push(1);
    let mut encoded = vec![0x84];
    text(&mut encoded, "eliot.revision-dek.selection.v1");
    encoded.extend_from_slice(&[1, 0, 0x88]);
    byte_string(&mut encoded, &[6; 16]);
    byte_string(&mut encoded, &candidate);
    byte_string(&mut encoded, &[config; 32]);
    byte_string(&mut encoded, &[8; 32]);
    byte_string(
        &mut encoded,
        core_secrets::section_descriptor().unwrap().field_registry_digest().as_bytes(),
    );
    text(&mut encoded, core_secrets::REVISION_DEK_SELECTION_BACKEND);
    text(&mut encoded, core_secrets::REVISION_DEK_SELECTION_SCOPE);
    encoded.push(11);
    let core_secrets::RevisionDekSelectionRecord::Intent(intent) =
        core_secrets::decode_revision_dek_selection_record(&encoded).unwrap()
    else {
        panic!("intent fixture");
    };
    intent
}

struct Context {
    refuse: bool,
    checkpoints: usize,
}

impl crate::LegacyRevisionDpapiContext for Context {
    type Error = &'static str;

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.checkpoints += 1;
        if self.refuse { Err("original-refusal") } else { Ok(()) }
    }

    fn charge(&mut self, _: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        panic!("no native or allocation work admitted by this fixture")
    }

    fn account_native_clear(&mut self, _: usize) -> Result<(), Self::Error> {
        panic!("no native output owned by this fixture")
    }
}

impl RevisionDekContext for Context {
    fn deadline(&self) -> Instant {
        panic!("original context refused before native work")
    }

    fn charge_open(&mut self) -> Result<(), Self::Error> {
        panic!("no handle operation admitted by this fixture")
    }
}

#[test]
fn original_refusal_is_returned_before_any_native_work() {
    let intent = intent(7, 7);
    let mut context = Context { refuse: true, checkpoints: 0 };
    assert!(matches!(
        construct_revision_dek_selection(intent.policy(), intent.candidate(), &mut context),
        Err(RevisionDekError::Context("original-refusal"))
    ));
    assert_eq!(context.checkpoints, 1);
}

#[test]
fn metadata_planning_requires_exact_retained_policy_and_candidate() {
    let original = intent(7, 7);
    let other_policy = intent(10, 7);
    let other_candidate = intent(7, 8);
    for different in [other_policy, other_candidate] {
        assert_eq!(
            prepare_matching_record(original.policy(), original.candidate(), &different),
            Err(RevisionDekSelectionError::InvalidIntent)
        );
    }
    // The planner is pure metadata, not a fabricated native dependency.
    let planned = prepare_matching_record(original.policy(), original.candidate(), &original).unwrap();
    assert_eq!(planned, RevisionDekCurrent::new(&original).unwrap());
    assert_eq!(planned.applied_policy_revision(), 1);
    assert_eq!(planned.selection_revision(), 1);
    assert_ne!(original.creator_epoch(), original.candidate().intent().creator_epoch());
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_cannot_return_a_dependency() {
    let intent = intent(7, 7);
    let mut context = Context { refuse: false, checkpoints: 0 };
    assert!(matches!(
        construct_revision_dek_selection(intent.policy(), intent.candidate(), &mut context),
        Err(RevisionDekError::UnsupportedPlatform)
    ));
    assert_eq!(context.checkpoints, 1);
}
