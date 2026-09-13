use super::*;
use crate::decode::{DecodedRepresentation, StepCounter, decode_text_or_code, detect_or_validate_encoding};
use crate::profile::{
    LossBehavior, NewlinePolicy, ValidatedMaterializerProfile, baseline_profile_descriptor,
    validate_materializer_profile,
};
use crate::request::{CancellationToken, DEFAULT_MATERIALIZATION_BUDGET};
use crate::{LineEnding, MaterializationError};

fn profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("normalize-test", 1))
        .expect("profile")
}

fn normalize_profile() -> ValidatedMaterializerProfile {
    let mut descriptor = baseline_profile_descriptor("normalize-lf", 1);
    descriptor.newline_policy = NewlinePolicy::NormalizeToLf;
    validate_materializer_profile(&descriptor).expect("normalize profile")
}

fn decoded_with(bytes: &[u8], profile: &ValidatedMaterializerProfile) -> DecodedRepresentation {
    let decision =
        detect_or_validate_encoding(bytes, crate::profile::SourceEncoding::Utf8, profile)
            .expect("decision");
    decode_text_or_code(
        bytes,
        &decision,
        profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("decode")
}

#[test]
fn preserve_exact_is_identity() {
    let profile = profile();
    let decoded = decoded_with(b"a\r\nb\nc\rd", &profile);
    let canonical = normalize_representation(
        &decoded,
        &profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("normalize");
    assert_eq!(canonical.text(), "a\r\nb\nc\rd");
    assert_eq!(canonical.lines().len(), 4);
    assert!(canonical.lines().iter().all(|line| !line.ending_changed()));
    assert_eq!(canonical.canonical_len_chars(), decoded.decoded_len_chars());
}

#[test]
fn crlf_and_cr_normalize_with_records() {
    let profile = normalize_profile();
    let decoded = decoded_with(b"a\r\nb\nc\rd", &profile);
    let canonical = normalize_representation(
        &decoded,
        &profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("normalize");
    assert_eq!(canonical.text(), "a\nb\nc\nd");
    assert_eq!(canonical.lines().len(), 4);
    assert!(canonical.lines()[0].ending_changed());
    assert!(!canonical.lines()[1].ending_changed());
    assert!(canonical.lines()[2].ending_changed());
    assert!(!canonical.lines()[3].ending_changed());
    assert_eq!(canonical.lines()[0].ending_after, LineEnding::Lf);
    assert_eq!(canonical.canonical_len_chars(), 7);
}

#[test]
fn strict_profile_rejects_newline_loss() {
    let mut descriptor = baseline_profile_descriptor("strict-normalize", 1);
    descriptor.newline_policy = NewlinePolicy::NormalizeToLf;
    descriptor.loss_behavior = LossBehavior::RejectOnAnyLoss;
    let strict = validate_materializer_profile(&descriptor).expect("strict");
    let decoded = decoded_with(b"a\r\n", &strict);
    assert_eq!(
        normalize_representation(
            &decoded,
            &strict,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::Loss)
    );
}

#[test]
fn foreign_profile_is_mismatch() {
    let profile = profile();
    let other = normalize_profile();
    let decoded = decoded_with(b"a\n", &profile);
    assert_eq!(
        normalize_representation(
            &decoded,
            &other,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::ProfileMismatch)
    );
}

#[test]
fn debug_never_leaks_canonical_text() {
    let profile = profile();
    let decoded = decoded_with(b"private-normalize\n", &profile);
    let canonical = normalize_representation(
        &decoded,
        &profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("normalize");
    assert!(!format!("{canonical:?}").contains("private-normalize"));
}
