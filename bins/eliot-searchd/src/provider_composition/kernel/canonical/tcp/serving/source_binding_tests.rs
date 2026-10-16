//! Source-binding dispatch regression. Full issuer/live-domain qualification is separate.

use super::*;
use search_contracts::{Blake3Digest32, OpaqueCanonicalBytes, SourceId, SourceIdentityKind,
    SourceNamespaceId, SourceRevisionId};
use std::collections::BTreeMap;

fn fixture() -> (SourceMembershipId, BTreeMap<SourceMembershipId, SourceIdentity>, SourceRevisionRef) {
    let membership = SourceMembershipId::from_bytes([1; 16]);
    let source = SourceIdentity {
        source_namespace_id: SourceNamespaceId::from_bytes([2; 16]),
        source_id: SourceId::from_bytes([3; 16]), identity_kind: SourceIdentityKind::NtfsFile,
        stable_identity_components: OpaqueCanonicalBytes::from_validated(vec![0x80]).unwrap(),
    };
    let expected = SourceRevisionRef { source_namespace_id: source.source_namespace_id,
        source_id: source.source_id, revision_id: SourceRevisionId::from_bytes([4; 16]),
        content_digest: Blake3Digest32::from_bytes([5; 32]), byte_length: 23 };
    (membership, BTreeMap::from([(membership, source)]), expected)
}

#[test]
fn foreign_or_missing_source_binding_never_dispatches_backend() {
    let (membership, sources, expected) = fixture();
    let mut foreign_namespace = expected;
    foreign_namespace.source_namespace_id = SourceNamespaceId::from_bytes([9; 16]);
    let mut foreign_source = expected;
    foreign_source.source_id = SourceId::from_bytes([9; 16]);
    for (bindings, selected, revision) in [
        (None, membership, expected),
        (Some(&sources), SourceMembershipId::from_bytes([9; 16]), expected),
        (Some(&sources), membership, foreign_namespace),
        (Some(&sources), membership, foreign_source),
    ] {
        let mut invoked = false;
        let result = with_bound_source(bindings, selected, &revision, |_| {
            invoked = true;
            Ok(())
        });
        assert!(matches!(result, Err(CanonicalServingError::Access(AccessError::ScopeUnauthorized))));
        assert!(!invoked, "ungranted source must be refused before I/O");
    }
}

#[test]
fn matching_source_dispatches_the_exact_revision_reference_once() {
    let (membership, sources, expected) = fixture();
    let mut calls = 0;
    let result = with_bound_source(Some(&sources), membership, &expected, |actual| {
        calls += 1;
        assert!(std::ptr::eq(actual, &expected));
        Err::<(), _>(CanonicalServingError::Backend("SOURCE_REVISION_UNAVAILABLE"))
    });
    assert!(matches!(result, Err(CanonicalServingError::Backend("SOURCE_REVISION_UNAVAILABLE"))));
    assert_eq!(calls, 1);
}
