//! T33 discriminating fixtures: authenticated ephemeral editor overlays.
//!
//! The [`search_overlay::OverlayStore`] is the bounded current-workspace
//! composition layer: `published base < saved revision < authenticated
//! unsaved snapshot`. Every test below proves one T33 exit obligation
//! without creating a durable second index:
//!
//! * dirty buffers shadow saved/published candidates before fusion;
//! * out-of-order edits, stale guards and conflicting editors fail closed;
//! * explicit save/admission is the only route from unsaved to saved;
//! * revocation, purge, workspace gaps and TTL invalidate before reads;
//! * unsaved bytes never escape into formatted views, digests or receipts;
//! * budget exhaustion and cancellation report explicit gaps and never
//!   unshadow stale base candidates;
//! * quota/TTL reduction invalidates excess state before the new receipt.

use std::collections::BTreeMap;

use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, BufferSnapshotId, OpaqueId, OpaqueRef,
    PositionEncoding, ProfileId, PurgeFenceRevision, ReceiptRef, SourceId, SourceMembershipId,
    SourceNamespaceId, SourceOwnerGeneration, SourceRevisionId, SourceRevisionRef, UtcTimestamp,
    WorkspaceViewRevisionId,
};
use search_overlay::{
    BaseNomination, CandidateInput, DEFAULT_UNSAVED_TTL_MILLIS, OverlayBinding, OverlayCandidate,
    OverlayError, OverlayGap, OverlayInvalidationCause, OverlayInvalidationScope, OverlayLimits,
    OverlayLiveState, OverlaySearchKind, OverlaySearchRequest, OverlayStore, PublishedBase,
    SavedOverlayStatus, UnsavedBufferGuard, UnsavedSnapshot,
};
use search_ports::{IdempotencyClass, MutationIdentity};

const CREATED_FIRST: &str = "2026-09-05T10:00:00.000000Z";
const CREATED_SECOND: &str = "2026-09-05T10:01:00.000000Z";
const VIEW_NOW: &str = "2026-09-05T10:02:00.000000Z";
const EXPIRES: &str = "2026-09-05T10:15:00.000000Z";

const fn membership(n: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(n.to_be_bytes())
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn oid(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("fixture opaque id must be valid")
}

fn oref(value: &str) -> OpaqueRef {
    OpaqueRef::new(value).expect("fixture opaque ref must be valid")
}

fn ts(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("fixture timestamp must be valid")
}

fn fixture_hash(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0x5A_u8; 32];
    let mut slot = 0_usize;
    for byte in bytes {
        out[slot] ^= *byte;
        slot = (slot + 1) % 32;
    }
    out
}

const fn binding_for(member: SourceMembershipId) -> OverlayBinding {
    OverlayBinding {
        binding_id: BindingId::from_bytes([0x33; 16]),
        source_namespace_id: SourceNamespaceId::from_bytes([0x11; 16]),
        source_id: SourceId::from_bytes([0x22; 16]),
        source_membership_id: member,
        source_owner_generation: SourceOwnerGeneration::from_bytes([0x44; 32]),
        access_policy_revision: AccessPolicyRevision::new(7),
        purge_fence_revision: PurgeFenceRevision::new(3),
        workspace_view_revision_id: WorkspaceViewRevisionId::from_bytes([0x55; 16]),
    }
}

const fn live_for(binding: OverlayBinding) -> OverlayLiveState {
    OverlayLiveState {
        binding_id: binding.binding_id,
        source_owner_generation: binding.source_owner_generation,
        access_policy_revision: binding.access_policy_revision,
        purge_fence_revision: binding.purge_fence_revision,
        workspace_view_revision_id: binding.workspace_view_revision_id,
        session_authenticated: true,
        overlay_permitted: true,
        purge_clear: true,
    }
}

fn live_map(bindings: &[OverlayBinding]) -> BTreeMap<SourceMembershipId, OverlayLiveState> {
    bindings
        .iter()
        .map(|binding| (binding.source_membership_id, live_for(*binding)))
        .collect()
}

fn unsaved_descriptor(
    member: SourceMembershipId,
    version: u64,
    snapshot_byte: u8,
    content: &[u8],
    created: &str,
) -> (UnsavedSnapshot, Vec<u8>) {
    let snapshot = UnsavedSnapshot {
        binding: binding_for(member),
        buffer_id: oid("buffer-editor-0001"),
        buffer_snapshot_id: BufferSnapshotId::from_bytes([snapshot_byte; 16]),
        buffer_version: version,
        position_encoding: PositionEncoding::Utf8Bytes,
        content_digest: digest(snapshot_byte ^ 0x40),
        byte_length: u64::try_from(content.len()).expect("fixture bytes must fit"),
        created_at: ts(created),
        expires_at: ts(EXPIRES),
        session_ref: oref("session-editor-0001"),
    };
    (snapshot, content.to_vec())
}

fn attach_fixture(
    store: &mut OverlayStore,
    member: SourceMembershipId,
    version: u64,
    snapshot_byte: u8,
    content: &[u8],
    created: &str,
    guard_byte: u8,
) -> UnsavedBufferGuard {
    let binding = binding_for(member);
    let live = live_for(binding);
    let (snapshot, bytes) = unsaved_descriptor(member, version, snapshot_byte, content, created);
    store
        .attach_unsaved_snapshot(
            snapshot,
            bytes,
            digest(guard_byte),
            DEFAULT_UNSAVED_TTL_MILLIS,
            live,
        )
        .expect("fixture attach must succeed")
}

fn admit_saved_fixture(
    store: &mut OverlayStore,
    member: SourceMembershipId,
    operation_tag: &str,
    operation_byte: u8,
) {
    let binding = binding_for(member);
    let live = live_for(binding);
    let revision = SourceRevisionRef {
        source_namespace_id: binding.source_namespace_id,
        source_id: binding.source_id,
        revision_id: SourceRevisionId::from_bytes([operation_byte; 16]),
        content_digest: digest(operation_byte ^ 0x80),
        byte_length: 128,
    };
    let operation = MutationIdentity::new(oid(operation_tag), IdempotencyClass::RetrySameIdentity);
    let receipt = store
        .admit_saved_overlay(
            binding,
            revision,
            ProfileId::new("fixture-profile").expect("fixture profile"),
            digest(0xE1),
            ReceiptRef::new("receipt-fixture-0001").expect("fixture receipt"),
            operation,
            digest(operation_byte),
            live,
        )
        .expect("fixture saved admission must succeed");
    assert!(!receipt.replayed);
    assert_eq!(receipt.entry.status, SavedOverlayStatus::Active);
}

fn replace_err(
    store: &mut OverlayStore,
    guard: &UnsavedBufferGuard,
    snapshot: UnsavedSnapshot,
    bytes: Vec<u8>,
    guard_byte: u8,
    live: OverlayLiveState,
) -> OverlayError {
    match store.replace_unsaved_snapshot(
        guard,
        snapshot,
        bytes,
        digest(guard_byte),
        DEFAULT_UNSAVED_TTL_MILLIS,
        live,
    ) {
        Err(error) => error,
        Ok(_) => panic!("conflicting replacement must fail"),
    }
}

fn search_request(pattern: &[u8], kind: OverlaySearchKind) -> OverlaySearchRequest {
    OverlaySearchRequest {
        kind,
        pattern: pattern.to_vec(),
        max_bytes: 1 << 20,
        max_steps: 1 << 20,
        max_candidates: 16,
    }
}

const fn published_fixture(member: SourceMembershipId, revision_byte: u8) -> PublishedBase {
    PublishedBase {
        source_membership_id: member,
        source_revision_ref: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([0x11; 16]),
            source_id: SourceId::from_bytes([0x22; 16]),
            revision_id: SourceRevisionId::from_bytes([revision_byte; 16]),
            content_digest: digest(revision_byte),
            byte_length: 64,
        },
    }
}

#[test]
fn dirty_buffer_shadows_saved_and_published() {
    let member = membership(1);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    admit_saved_fixture(&mut store, member, "operation-t33-shadow-0001", 0x71);
    let content = b"dirty buffer needle alpha dirty";
    attach_fixture(&mut store, member, 1, 0xA1, content, CREATED_FIRST, 0xB1);

    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("authorized view must succeed");
    assert_eq!(view.entries.len(), 2);

    let shadows = store
        .compute_shadow_set(&[published_fixture(member, 0x01)], &view)
        .expect("shadow set must compute");
    assert_eq!(shadows.targets.len(), 2);
    assert!(shadows.fail_closed_memberships.is_empty());

    let candidates = store
        .retrieve_overlay(
            &search_request(b"needle", OverlaySearchKind::ExactLiteral),
            &view,
            &search_overlay::NeverCancelled,
        )
        .expect("bounded retrieval must succeed");
    let ranges: Vec<(u64, u64)> = candidates
        .candidates
        .iter()
        .filter_map(|candidate| match candidate {
            OverlayCandidate::UnsavedMatch { range, .. } => {
                Some((range.byte_start, range.byte_end))
            }
            OverlayCandidate::SavedReference { .. } => None,
        })
        .collect();
    assert_eq!(ranges, vec![(13, 19)]);
    assert!(
        candidates
            .gaps
            .iter()
            .any(|gap| *gap == OverlayGap::SavedReadRequired)
    );
    assert!(!candidates.complete);

    let base = vec![BaseNomination {
        published: published_fixture(member, 0x01),
        rank_key: digest(0xD1),
    }];
    let merged = store
        .merge_overlay_and_base(base, &candidates, &shadows)
        .expect("fusion must succeed");
    assert!(!merged.is_empty());
    assert!(
        merged
            .iter()
            .all(|input| matches!(input, CandidateInput::Overlay(_))),
        "shadowed published base must not survive fusion"
    );
}

#[test]
fn token_kind_requires_delimiter_boundaries() {
    let member = membership(2);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xA2,
        b"needle needless",
        CREATED_FIRST,
        0xB2,
    );
    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("authorized view must succeed");

    let token = store
        .retrieve_overlay(
            &search_request(b"needle", OverlaySearchKind::Token),
            &view,
            &search_overlay::NeverCancelled,
        )
        .expect("token retrieval must succeed");
    assert_eq!(token.candidates.len(), 1);

    let fragment = store
        .retrieve_overlay(
            &search_request(b"eedl", OverlaySearchKind::Token),
            &view,
            &search_overlay::NeverCancelled,
        )
        .expect("fragment retrieval must succeed");
    assert!(fragment.candidates.is_empty());

    let literal = store
        .retrieve_overlay(
            &search_request(b"eedl", OverlaySearchKind::ExactLiteral),
            &view,
            &search_overlay::NeverCancelled,
        )
        .expect("literal retrieval must succeed");
    assert_eq!(literal.candidates.len(), 2);
}

#[test]
fn out_of_order_edit_and_stale_guard_fail_closed() {
    let member = membership(3);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    let first = attach_fixture(
        &mut store,
        member,
        1,
        0xA3,
        b"version one",
        CREATED_FIRST,
        0xB3,
    );

    let binding = binding_for(member);
    let live = live_for(binding);
    let (second_snapshot, second_bytes) =
        unsaved_descriptor(member, 2, 0xA4, b"version two", CREATED_SECOND);
    let receipt = match store.replace_unsaved_snapshot(
        &first,
        second_snapshot,
        second_bytes,
        digest(0xB4),
        DEFAULT_UNSAVED_TTL_MILLIS,
        live,
    ) {
        Ok(receipt) => receipt,
        Err(error) => panic!("forward replacement must succeed: {error:?}"),
    };
    assert_eq!(
        receipt.replaced_snapshot_id,
        BufferSnapshotId::from_bytes([0xA3; 16])
    );
    let current = receipt.guard;

    let regress = unsaved_descriptor(member, 1, 0xA5, b"version one again", CREATED_SECOND).0;
    assert_eq!(
        replace_err(
            &mut store,
            &current,
            regress,
            b"version one again".to_vec(),
            0xB5,
            live_for(binding),
        ),
        OverlayError::UnsavedVersionConflict
    );

    let same_id = unsaved_descriptor(member, 3, 0xA4, b"version three", CREATED_SECOND).0;
    assert_eq!(
        replace_err(
            &mut store,
            &current,
            same_id,
            b"version three".to_vec(),
            0xB6,
            live_for(binding),
        ),
        OverlayError::UnsavedVersionConflict
    );

    let (third_snapshot, third_bytes) =
        unsaved_descriptor(member, 3, 0xA6, b"version three", CREATED_SECOND);
    assert_eq!(
        replace_err(
            &mut store,
            &first,
            third_snapshot,
            third_bytes,
            0xB7,
            live_for(binding),
        ),
        OverlayError::OverlayGuardMismatch
    );

    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view after failed replacements must succeed");
    assert_eq!(view.entries.len(), 1);
}

#[test]
fn conflicting_buffer_identity_is_rejected() {
    let member = membership(4);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xA7,
        b"first editor",
        CREATED_FIRST,
        0xB8,
    );

    let mut intruder = unsaved_descriptor(member, 1, 0xA8, b"second editor", CREATED_SECOND).0;
    intruder.buffer_id = oid("buffer-editor-0002");
    assert_eq!(
        store
            .attach_unsaved_snapshot(
                intruder,
                b"second editor".to_vec(),
                digest(0xB9),
                DEFAULT_UNSAVED_TTL_MILLIS,
                live_for(binding_for(member)),
            )
            .expect_err("conflicting editor must fail"),
        OverlayError::UnsavedVersionConflict
    );
}

#[test]
fn save_transition_requires_matching_durable_receipt() {
    let member = membership(5);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    let content = b"save me exactly";
    let guard = attach_fixture(&mut store, member, 5, 0xA9, content, CREATED_FIRST, 0xBA);
    let content_digest = digest(0xA9 ^ 0x40);
    let byte_length = u64::try_from(content.len()).expect("fixture bytes must fit");

    let observed = SourceRevisionRef {
        source_namespace_id: binding.source_namespace_id,
        source_id: binding.source_id,
        revision_id: SourceRevisionId::from_bytes([0xC9; 16]),
        content_digest,
        byte_length,
    };
    let transition = store
        .prepare_save_admission(
            &guard,
            observed,
            ReceiptRef::new("receipt-save-0001").expect("fixture receipt"),
            live_for(binding),
        )
        .expect("matching save proof must succeed");

    let mismatched = SourceRevisionRef {
        source_namespace_id: binding.source_namespace_id,
        source_id: binding.source_id,
        revision_id: SourceRevisionId::from_bytes([0xCA; 16]),
        content_digest: digest(0xFF),
        byte_length,
    };
    assert_eq!(
        store
            .prepare_save_admission(
                &guard,
                mismatched,
                ReceiptRef::new("receipt-save-0002").expect("fixture receipt"),
                live_for(binding),
            )
            .expect_err("digest mismatch must fail"),
        OverlayError::OverlaySaveConflict
    );

    let receipt = store
        .commit_save_transition(&guard, &transition)
        .expect("commit must retire the exact snapshot");
    assert_eq!(receipt.cause, OverlayInvalidationCause::SavedTransition);
    assert_eq!(receipt.invalidated_snapshot_ids.len(), 1);

    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view after save must succeed");
    assert!(view.entries.is_empty());
    assert_eq!(
        store
            .commit_save_transition(&guard, &transition)
            .expect_err("replayed commit must fail"),
        OverlayError::OverlayGuardMismatch
    );
}

#[test]
fn reconnect_and_revocation_fail_closed() {
    let member = membership(6);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xAA,
        b"session bytes",
        CREATED_FIRST,
        0xBB,
    );

    let mut foreign = live_for(binding);
    foreign.binding_id = BindingId::from_bytes([0x99; 16]);
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, foreign)]),
                &ts(VIEW_NOW),
                fixture_hash,
            )
            .expect_err("cross-binding view must fail"),
        OverlayError::OverlayAuthorizationLost
    );

    let mut unauthenticated = live_for(binding);
    unauthenticated.session_authenticated = false;
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, unauthenticated)]),
                &ts(VIEW_NOW),
                fixture_hash,
            )
            .expect_err("unauthenticated view must fail"),
        OverlayError::UnsavedBufferUnauthenticated
    );

    let mut revoked = live_for(binding);
    revoked.overlay_permitted = false;
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, revoked)]),
                &ts(VIEW_NOW),
                fixture_hash
            )
            .expect_err("revoked grant must fail"),
        OverlayError::OverlayAuthorizationLost
    );
}

#[test]
fn purge_owner_change_and_policy_drift_fail_closed() {
    let member = membership(7);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xAB,
        b"fenced bytes",
        CREATED_FIRST,
        0xBC,
    );

    let mut purged = live_for(binding);
    purged.purge_clear = false;
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, purged)]),
                &ts(VIEW_NOW),
                fixture_hash
            )
            .expect_err("purge barrier must fail"),
        OverlayError::OverlayPurged
    );

    let mut rotated = live_for(binding);
    rotated.source_owner_generation = SourceOwnerGeneration::from_bytes([0xEE; 32]);
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, rotated)]),
                &ts(VIEW_NOW),
                fixture_hash
            )
            .expect_err("owner rotation must fail"),
        OverlayError::OverlayOwnerGenerationChanged
    );

    let mut drifted = live_for(binding);
    drifted.access_policy_revision = AccessPolicyRevision::new(8);
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, drifted)]),
                &ts(VIEW_NOW),
                fixture_hash
            )
            .expect_err("policy drift must fail"),
        OverlayError::OverlayAuthorizationLost
    );

    let mut gap = live_for(binding);
    gap.workspace_view_revision_id = WorkspaceViewRevisionId::from_bytes([0x60; 16]);
    assert_eq!(
        store
            .snapshot_overlay_view(
                &BTreeMap::from([(member, gap)]),
                &ts(VIEW_NOW),
                fixture_hash
            )
            .expect_err("workspace gap must fail"),
        OverlayError::UnsavedVersionConflict
    );
}

#[test]
fn missing_live_state_is_an_authorization_gap() {
    let member = membership(8);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xAC,
        b"gapped bytes",
        CREATED_FIRST,
        0xBD,
    );
    assert_eq!(
        store
            .snapshot_overlay_view(&BTreeMap::new(), &ts(VIEW_NOW), fixture_hash)
            .expect_err("missing live state must fail"),
        OverlayError::OverlayAuthorizationLost
    );
    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("restored live state must succeed");
    assert_eq!(view.entries.len(), 1);
}

#[test]
fn ttl_expiry_evicts_before_snapshot_and_stales_views() {
    let member = membership(9);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xAD,
        b"expiring bytes",
        CREATED_FIRST,
        0xBE,
    );
    let revision_before = store.revision();

    let fresh = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("unexpired view must succeed");
    assert_eq!(fresh.entries.len(), 1);

    let evicted = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(EXPIRES), fixture_hash)
        .expect("expiry pass must succeed");
    assert!(evicted.entries.is_empty());
    assert!(store.revision().get() > revision_before.get());

    assert_eq!(
        store
            .retrieve_overlay(
                &search_request(b"expiring", OverlaySearchKind::ExactLiteral),
                &fresh,
                &search_overlay::NeverCancelled,
            )
            .expect_err("stale view must fail"),
        OverlayError::OverlayPrecedenceUnknown
    );
}

#[test]
fn unsaved_bytes_never_escape_formatted_views() {
    let member = membership(10);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    let token = "7f3a9c-ephemeral-unsaved-sentinel";
    let mut content = Vec::from(b"prefix ".as_slice());
    content.extend_from_slice(token.as_bytes());
    content.extend_from_slice(b" suffix");
    let guard = attach_fixture(&mut store, member, 1, 0xAE, &content, CREATED_FIRST, 0xBF);

    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view must succeed");
    let candidates = store
        .retrieve_overlay(
            &search_request(b"7f3a9c", OverlaySearchKind::ExactLiteral),
            &view,
            &search_overlay::NeverCancelled,
        )
        .expect("retrieval must succeed");
    assert_eq!(candidates.candidates.len(), 1);
    let shadows = store
        .compute_shadow_set(&[], &view)
        .expect("shadow set must succeed");

    let store_text = format!("{store:?}");
    let guard_text = format!("{guard:?}");
    let view_text = format!("{view:?}");
    let candidates_text = format!("{candidates:?}");
    let shadows_text = format!("{shadows:?}");
    for rendered in [
        &store_text,
        &guard_text,
        &view_text,
        &candidates_text,
        &shadows_text,
    ] {
        assert!(
            !rendered.contains(token),
            "unsaved bytes escaped into a formatted view: {rendered}"
        );
    }
    assert!(
        store_text.contains("<redacted"),
        "memory-only bytes must render redacted: {store_text}"
    );
    assert!(
        guard_text.contains("<redacted>"),
        "guard digest must render redacted: {guard_text}"
    );

    let receipt = store
        .close_or_invalidate_unsaved(
            &OverlayInvalidationScope::Snapshot(BufferSnapshotId::from_bytes([0xAE; 16])),
            OverlayInvalidationCause::EditorClose,
        )
        .expect("close must succeed");
    assert_eq!(receipt.invalidated_snapshot_ids.len(), 1);
    let closed = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view after close must succeed");
    assert!(closed.entries.is_empty());
    assert_eq!(store.unsaved_bytes(), 0);
}

#[test]
fn budget_and_cancellation_keep_shadows_explicit() {
    let member = membership(11);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(
        &mut store,
        member,
        1,
        0xAF,
        b"needle needle needle",
        CREATED_FIRST,
        0xC0,
    );
    let view = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view must succeed");
    let shadows = store
        .compute_shadow_set(&[published_fixture(member, 0x02)], &view)
        .expect("shadows must compute");
    assert_eq!(shadows.targets.len(), 1);

    let starved = OverlaySearchRequest {
        kind: OverlaySearchKind::ExactLiteral,
        pattern: b"needle".to_vec(),
        max_bytes: 4,
        max_steps: 1 << 20,
        max_candidates: 16,
    };
    let partial = store
        .retrieve_overlay(&starved, &view, &search_overlay::NeverCancelled)
        .expect("starved retrieval must stay typed");
    assert!(partial.candidates.is_empty());
    assert!(!partial.complete);
    assert!(
        partial
            .gaps
            .iter()
            .any(|gap| *gap == OverlayGap::BudgetExhausted)
    );

    let cancelled = store
        .retrieve_overlay(
            &search_request(b"needle", OverlaySearchKind::ExactLiteral),
            &view,
            &Cancelling,
        )
        .expect("cancelled retrieval must stay typed");
    assert!(cancelled.candidates.is_empty());
    assert!(!cancelled.complete);
    assert!(
        cancelled
            .gaps
            .iter()
            .any(|gap| *gap == OverlayGap::Cancelled)
    );

    let base = vec![BaseNomination {
        published: published_fixture(member, 0x02),
        rank_key: digest(0xD2),
    }];
    let merged = store
        .merge_overlay_and_base(base, &partial, &shadows)
        .expect("fusion must succeed");
    assert!(
        merged
            .iter()
            .all(|input| matches!(input, CandidateInput::Overlay(_))),
        "partial retrieval must never unshadow stale base"
    );
}

struct Cancelling;

impl search_overlay::OverlayCancellation for Cancelling {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[test]
fn quota_reduction_invalidates_oldest_before_receipt() {
    let first = membership(12);
    let second = membership(13);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(&mut store, first, 1, 0xB0, &[b'x'; 32], CREATED_FIRST, 0xC1);
    attach_fixture(
        &mut store,
        second,
        1,
        0xB1,
        &[b'y'; 40],
        CREATED_SECOND,
        0xC2,
    );
    assert_eq!(store.unsaved_bytes(), 72);

    let reduced = OverlayLimits {
        max_unsaved_bytes_per_snapshot: 40,
        max_unsaved_bytes_total: 48,
        ..OverlayLimits::BASELINE
    };
    let receipt = store
        .apply_live_limits(reduced)
        .expect("quota reduction must succeed");
    assert_eq!(receipt.cause, OverlayInvalidationCause::QuotaReduction);
    assert_eq!(receipt.invalidated_snapshot_ids.len(), 1);
    assert_eq!(
        receipt.invalidated_snapshot_ids.as_slice(),
        &[BufferSnapshotId::from_bytes([0xB0; 16])]
    );
    assert_eq!(store.limits().max_unsaved_bytes_total, 48);
    assert_eq!(store.unsaved_bytes(), 40);

    let bindings = [binding_for(first), binding_for(second)];
    let view = store
        .snapshot_overlay_view(&live_map(&bindings), &ts(VIEW_NOW), fixture_hash)
        .expect("view after reduction must succeed");
    assert_eq!(view.entries.len(), 1);

    let restored = OverlayLimits::BASELINE;
    store
        .apply_live_limits(restored)
        .expect("raising limits must succeed");
    let revived = store
        .snapshot_overlay_view(&live_map(&bindings), &ts(VIEW_NOW), fixture_hash)
        .expect("view after raise must succeed");
    assert_eq!(
        revived.entries.len(),
        1,
        "raising limits must not resurrect expired state"
    );
}

#[test]
fn close_scopes_are_idempotent_and_deterministic() {
    let first = membership(14);
    let second = membership(15);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    attach_fixture(&mut store, first, 1, 0xB2, b"first", CREATED_FIRST, 0xC3);
    attach_fixture(&mut store, second, 1, 0xB3, b"second", CREATED_SECOND, 0xC4);

    let one = store
        .close_or_invalidate_unsaved(
            &OverlayInvalidationScope::Membership(first),
            OverlayInvalidationCause::EditorClose,
        )
        .expect("scoped close must succeed");
    assert_eq!(one.invalidated_snapshot_ids.len(), 1);
    assert!(!one.more_remaining);

    let rest = store
        .close_or_invalidate_unsaved(
            &OverlayInvalidationScope::All,
            OverlayInvalidationCause::DaemonShutdown,
        )
        .expect("close-all must succeed");
    assert_eq!(rest.invalidated_snapshot_ids.len(), 1);

    let empty = store
        .close_or_invalidate_unsaved(
            &OverlayInvalidationScope::All,
            OverlayInvalidationCause::DaemonShutdown,
        )
        .expect("repeated close must succeed");
    assert!(empty.invalidated_snapshot_ids.is_empty());
    assert!(!empty.more_remaining);
}

#[test]
fn saved_admission_is_idempotent_and_conflict_bound() {
    let member = membership(16);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    let revision = SourceRevisionRef {
        source_namespace_id: binding.source_namespace_id,
        source_id: binding.source_id,
        revision_id: SourceRevisionId::from_bytes([0xD0; 16]),
        content_digest: digest(0xD0),
        byte_length: 64,
    };
    let operation = MutationIdentity::new(
        oid("operation-t33-saved-0001"),
        IdempotencyClass::RetrySameIdentity,
    );
    let first = store
        .admit_saved_overlay(
            binding,
            revision,
            ProfileId::new("fixture-profile").expect("fixture profile"),
            digest(0xE2),
            ReceiptRef::new("receipt-fixture-0002").expect("fixture receipt"),
            operation.clone(),
            digest(0xD1),
            live_for(binding),
        )
        .expect("first admission must succeed");
    assert!(!first.replayed);

    let replay = store
        .admit_saved_overlay(
            binding,
            revision,
            ProfileId::new("fixture-profile").expect("fixture profile"),
            digest(0xE2),
            ReceiptRef::new("receipt-fixture-0002").expect("fixture receipt"),
            operation.clone(),
            digest(0xD1),
            live_for(binding),
        )
        .expect("same operation and digest must replay");
    assert!(replay.replayed);
    assert_eq!(replay.entry.overlay_revision, first.entry.overlay_revision);

    assert_eq!(
        store
            .admit_saved_overlay(
                binding,
                revision,
                ProfileId::new("fixture-profile").expect("fixture profile"),
                digest(0xE2),
                ReceiptRef::new("receipt-fixture-0002").expect("fixture receipt"),
                operation,
                digest(0xD2),
                live_for(binding),
            )
            .expect_err("changed digest must conflict"),
        OverlayError::OverlayOperationConflict
    );

    let foreign = SourceRevisionRef {
        source_namespace_id: SourceNamespaceId::from_bytes([0x77; 16]),
        ..revision
    };
    assert_eq!(
        store
            .admit_saved_overlay(
                binding,
                foreign,
                ProfileId::new("fixture-profile").expect("fixture profile"),
                digest(0xE2),
                ReceiptRef::new("receipt-fixture-0003").expect("fixture receipt"),
                MutationIdentity::new(
                    oid("operation-t33-saved-0002"),
                    IdempotencyClass::RetrySameIdentity
                ),
                digest(0xD3),
                live_for(binding),
            )
            .expect_err("foreign namespace must fail"),
        OverlayError::OverlayPrecedenceUnknown
    );
}

#[test]
fn restart_recovers_saved_only_and_drops_unsaved() {
    let member = membership(17);
    let binding = binding_for(member);
    let mut store = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    admit_saved_fixture(&mut store, member, "operation-t33-restart-0001", 0x72);
    attach_fixture(
        &mut store,
        member,
        1,
        0xB4,
        b"volatile bytes",
        CREATED_FIRST,
        0xC5,
    );

    let saved_entry = store
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("view must succeed")
        .entries
        .iter()
        .find_map(|entry| match entry {
            search_overlay::OverlayViewEntry::Saved { revision, .. } => Some(*revision),
            search_overlay::OverlayViewEntry::Unsaved { .. } => None,
        })
        .expect("saved entry must be visible");

    let mut restarted = OverlayStore::new(OverlayLimits::BASELINE).expect("baseline limits");
    let recovered = restarted
        .recover_saved_overlay(
            vec![search_overlay::SavedOverlayEntry {
                binding,
                revision: saved_entry,
                preparation_profile_id: ProfileId::new("fixture-profile").expect("fixture profile"),
                preparation_digest: digest(0xE1),
                revision_receipt_ref: ReceiptRef::new("receipt-fixture-0001")
                    .expect("fixture receipt"),
                overlay_revision: search_contracts::OverlayRevision::new(1),
                status: SavedOverlayStatus::Active,
                operation: MutationIdentity::new(
                    oid("operation-t33-restart-0001"),
                    IdempotencyClass::RetrySameIdentity,
                ),
                operation_request_digest: digest(0x72),
            }],
            &live_map(&[binding]),
        )
        .expect("saved recovery must succeed");
    assert_eq!(recovered.len(), 1);
    assert_eq!(restarted.unsaved_bytes(), 0);

    let view = restarted
        .snapshot_overlay_view(&live_map(&[binding]), &ts(VIEW_NOW), fixture_hash)
        .expect("recovered view must succeed");
    assert!(
        view.entries
            .iter()
            .all(|entry| matches!(entry, search_overlay::OverlayViewEntry::Saved { .. })),
        "restart must never recreate unsaved content"
    );
}

#[test]
fn overlay_error_codes_are_stable() {
    assert_eq!(
        OverlayError::UnsavedBufferUnobserved.code(),
        "UNSAVED_BUFFER_UNOBSERVED"
    );
    assert_eq!(
        OverlayError::UnsavedBufferUnauthenticated.code(),
        "UNSAVED_BUFFER_UNAUTHENTICATED"
    );
    assert_eq!(OverlayError::OverlayExpired.code(), "OVERLAY_EXPIRED");
    assert_eq!(OverlayError::OverlayPurged.code(), "OVERLAY_PURGED");
    assert_eq!(
        OverlayError::DurableUnsavedForbidden.code(),
        "DURABLE_UNSAVED_FORBIDDEN"
    );
}
