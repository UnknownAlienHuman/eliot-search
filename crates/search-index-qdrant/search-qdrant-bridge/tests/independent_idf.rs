//! T22 independent-IDF profile: failing-first contract tests.
//!
//! Frozen lexical profiles delegate IDF to Qdrant (`IdfMode::DelegatedToQdrant`,
//! local factor exactly 1.0, no local statistics). The bridge must admit only
//! that shape: Qdrant-side `idf` modifier enabled, retrieval filter and
//! `idf.corpus` sharing one canonical base eligibility plan. Any local IDF
//! contribution is double IDF and must fail admission.

use search_qdrant_bridge::qualified::{
    BaseEligibility, IndependentIdfProfile, QualificationError, admit_independent_idf,
};

fn eligibility(tenant: &str) -> BaseEligibility {
    BaseEligibility {
        access_partition: "partition-a".to_owned(),
        tenant: tenant.to_owned(),
        visible_epoch: 42,
    }
}

fn delegated_profile() -> IndependentIdfProfile {
    let plan = eligibility("tenant-a");
    IndependentIdfProfile {
        vector_name: "lex_code_v1".to_owned(),
        local_idf_factor: 1.0,
        local_statistics_present: false,
        qdrant_modifier_idf: true,
        retrieval_eligibility: plan.clone(),
        idf_corpus_eligibility: plan,
    }
}

#[test]
fn delegated_idf_with_shared_plan_admits() {
    let receipt = admit_independent_idf(&delegated_profile()).expect("delegated IDF admits");
    assert_eq!(receipt.vector_name, "lex_code_v1");
    assert!(receipt.double_idf_impossible);
}

#[test]
fn local_idf_factor_is_double_idf() {
    let mut profile = delegated_profile();
    profile.local_idf_factor = 1.37;
    assert_eq!(
        admit_independent_idf(&profile).expect_err("local IDF factor must reject"),
        QualificationError::DoubleIdf
    );
}

#[test]
fn local_statistics_are_double_idf() {
    let mut profile = delegated_profile();
    profile.local_statistics_present = true;
    assert_eq!(
        admit_independent_idf(&profile).expect_err("local statistics must reject"),
        QualificationError::DoubleIdf
    );
}

#[test]
fn missing_qdrant_modifier_rejected() {
    let mut profile = delegated_profile();
    profile.qdrant_modifier_idf = false;
    assert_eq!(
        admit_independent_idf(&profile).expect_err("modifier off must reject"),
        QualificationError::IdfModifierMissing
    );
}

#[test]
fn diverged_idf_corpus_rejected() {
    // The IDF population filter is specified independently, but it must
    // share the exact base eligibility plan; a narrowed/widened corpus
    // would silently change denominators.
    let mut profile = delegated_profile();
    profile.idf_corpus_eligibility = eligibility("tenant-b");
    assert_eq!(
        admit_independent_idf(&profile).expect_err("diverged corpus must reject"),
        QualificationError::CorpusEligibilityDiverged
    );
}

#[test]
fn diverged_epoch_rejected() {
    let mut profile = delegated_profile();
    profile.idf_corpus_eligibility.visible_epoch = 43;
    assert_eq!(
        admit_independent_idf(&profile).expect_err("diverged epoch must reject"),
        QualificationError::CorpusEligibilityDiverged
    );
}

#[test]
fn empty_vector_name_rejected() {
    let mut profile = delegated_profile();
    profile.vector_name.clear();
    assert_eq!(
        admit_independent_idf(&profile).expect_err("empty vector name must reject"),
        QualificationError::InvalidVectorName
    );
}
