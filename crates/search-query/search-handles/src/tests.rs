use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use search_contracts::{
    AssuranceClass, BindingId, Blake3Digest32, Blake3ExactInputLimit, BoundedSet,
    BufferRangeAnchor, BufferSnapshotId, DisclosureCeiling, GrantId, HandleId, HandleTokenDigest,
    MAX_BLAKE3_EXACT_BYTES, MAX_SET_ITEMS, NativeAnchor, ObjectResidencyKeyDigest,
    OpaqueHandleToken, OwnerEpoch, PositionEncoding, ProfileId, RepresentationId,
    SearchSourceHandle, SearchSourceHandleRecord, SourceId, SourceMembershipId, SourceNamespaceId,
    SourceOwnerGeneration, SourceRevisionId, SourceRevisionRef, SourceView, TextBytesAnchor,
    UnitId, UtcTimestamp, WorkspaceId, WorkspaceViewRevisionId, blake3_exact_bytes,
};
use search_ports::PackageOpaque;

use crate::{
    EphemeralHandleTarget, HandleAuthorizationCheckpoint, HandleAuthorizationPhase,
    HandleDeliveryError, HandleError, HandleExpansionPermit, HandleInvalidationScope, HandlePolicy,
    HandleReadback, HandleReadbackPort, HandleReauthorizationPort, HandleStore, MintMaterial,
    RetainedSourceProtectionGuard, RetainedSourceProtectionPort, RetainedSourceTarget,
    UnsavedBufferTarget, target_excerpt_digest,
};

fn digest(bytes: &[u8]) -> Blake3Digest32 {
    blake3_exact_bytes(bytes, Blake3ExactInputLimit::new(bytes.len()).unwrap()).unwrap()
}

fn target() -> RetainedSourceTarget {
    let source = b"prefix|body|suffix";
    let source_namespace_id = SourceNamespaceId::from_bytes([1; 16]);
    let source_id = SourceId::from_bytes([2; 16]);
    let revision_id = SourceRevisionId::from_bytes([3; 16]);
    let content_digest = digest(source);
    RetainedSourceTarget {
        source_namespace_id,
        source_owner_generation: SourceOwnerGeneration::from_bytes([4; 32]),
        source_revision_ref: SourceRevisionRef {
            source_namespace_id,
            source_id,
            revision_id,
            content_digest,
            byte_length: u64::try_from(source.len()).unwrap(),
        },
        source_view: SourceView::RetainedRevision(revision_id),
        workspace_view_revision_ref: None,
        native_anchor: NativeAnchor::TextBytes(TextBytesAnchor {
            content_digest,
            byte_start_0: 7,
            byte_end_exclusive_0: 11,
        }),
        excerpt_digest: digest(b"body"),
        materialization_profile_id: ProfileId::new("materializer-test-v1").unwrap(),
        assurance_ceiling: AssuranceClass::ExactBytes,
        object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([5; 32]),
    }
}

fn buffer_target(snapshot: u8) -> EphemeralHandleTarget {
    let buffer_snapshot_id = BufferSnapshotId::from_bytes([snapshot; 16]);
    EphemeralHandleTarget::UnsavedBuffer(UnsavedBufferTarget {
        workspace_id: WorkspaceId::from_bytes([12; 16]),
        workspace_view_revision_ref: WorkspaceViewRevisionId::from_bytes([13; 16]),
        buffer_snapshot_id,
        buffer_version: 1,
        native_anchor: NativeAnchor::BufferRange(BufferRangeAnchor {
            buffer_snapshot_id,
            buffer_version: 1,
            position_encoding: PositionEncoding::Utf8Bytes,
            start_line_0: 0,
            start_character_0: 0,
            end_line_0: 0,
            end_character_0: 4,
        }),
        excerpt_digest: digest(b"body"),
    })
}

fn binding() -> (BindingId, GrantId, OwnerEpoch, DisclosureCeiling) {
    (
        BindingId::from_bytes([6; 16]),
        GrantId::from_bytes([7; 16]),
        OwnerEpoch::new(8).unwrap(),
        DisclosureCeiling::NamedClient,
    )
}

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).unwrap()
}

struct TestCrypto {
    next: u8,
}

impl TestCrypto {
    fn digest(token: &OpaqueHandleToken) -> HandleTokenDigest {
        let mut input = b"test-only/handle-token/v1".to_vec();
        input.extend_from_slice(token.as_bytes());
        let digest = digest(&input);
        HandleTokenDigest::from_stored_bytes(*digest.as_bytes())
    }
}

impl crate::HandleCryptoPort for TestCrypto {
    type Error = ();

    fn mint_material(&mut self) -> Result<MintMaterial, Self::Error> {
        self.next = self.next.saturating_add(1);
        let opaque_token = OpaqueHandleToken::new(&[self.next; 32]).unwrap();
        Ok(MintMaterial {
            handle_id: HandleId::from_bytes([self.next; 16]),
            token_digest: Self::digest(&opaque_token),
            opaque_token,
            created_at: timestamp("2026-10-16T10:00:00.000000Z"),
            expires_at: timestamp("2026-10-16T10:05:00.000000Z"),
            monotonic_deadline: Instant::now() + Duration::from_secs(300),
        })
    }

    fn token_digest(&self, token: &OpaqueHandleToken) -> Result<HandleTokenDigest, Self::Error> {
        Ok(Self::digest(token))
    }
}

struct TestGuard {
    target: RetainedSourceTarget,
    expires_at: UtcTimestamp,
    monotonic_deadline: Instant,
    drops: Arc<AtomicUsize>,
}

impl std::fmt::Debug for TestGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TestGuard(<redacted>)")
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl PackageOpaque for TestGuard {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

impl RetainedSourceProtectionGuard for TestGuard {
    fn target(&self) -> &RetainedSourceTarget {
        &self.target
    }

    fn expires_at(&self) -> &UtcTimestamp {
        &self.expires_at
    }

    fn monotonic_deadline(&self) -> Instant {
        self.monotonic_deadline
    }

    fn revalidate_current(&self) -> Result<(), HandleError> {
        Ok(())
    }
}

#[derive(Debug)]
struct TestProtection {
    available: bool,
    wrong_deadline: bool,
    drops: Arc<AtomicUsize>,
}

impl PackageOpaque for TestProtection {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

impl RetainedSourceProtectionPort for TestProtection {
    type Guard = TestGuard;

    fn protect(
        &mut self,
        target: &RetainedSourceTarget,
        handle_expires_at: &UtcTimestamp,
        handle_monotonic_deadline: Instant,
    ) -> Result<Self::Guard, HandleError> {
        if !self.available {
            return Err(HandleError::SourceProtectionUnavailable);
        }
        Ok(TestGuard {
            target: target.clone(),
            expires_at: handle_expires_at.clone(),
            monotonic_deadline: if self.wrong_deadline {
                handle_monotonic_deadline + Duration::from_millis(1)
            } else {
                handle_monotonic_deadline
            },
            drops: Arc::clone(&self.drops),
        })
    }
}

#[derive(Debug)]
struct TestCheckpoint;

impl PackageOpaque for TestCheckpoint {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

impl HandleAuthorizationCheckpoint for TestCheckpoint {
    fn revalidate(
        &self,
        record: &SearchSourceHandleRecord,
        source_membership_id: Option<SourceMembershipId>,
        representation_id: Option<RepresentationId>,
        unit_id: Option<UnitId>,
        owner_epoch: OwnerEpoch,
        disclosure_ceiling: DisclosureCeiling,
    ) -> Result<(), HandleError> {
        let SearchSourceHandleRecord::Ephemeral(record) = record else {
            return Err(HandleError::HandleNotFound);
        };
        if record.status != search_contracts::lifecycle::LifecycleRecordStatus::Active
            || source_membership_id != Some(SourceMembershipId::from_bytes([9; 16]))
            || representation_id != Some(RepresentationId::from_bytes([10; 16]))
            || unit_id != Some(UnitId::from_bytes([11; 16]))
            || owner_epoch != OwnerEpoch::new(8).unwrap()
            || disclosure_ceiling != DisclosureCeiling::NamedClient
        {
            return Err(HandleError::AuthorizationDenied);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct TestAuthorization;

impl PackageOpaque for TestAuthorization {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

impl HandleReauthorizationPort for TestAuthorization {
    type Checkpoint = TestCheckpoint;

    fn checkpoint(
        &mut self,
        _record: &SearchSourceHandleRecord,
        _source_membership_id: Option<SourceMembershipId>,
        _representation_id: Option<RepresentationId>,
        _unit_id: Option<UnitId>,
        _owner_epoch: OwnerEpoch,
        _disclosure_ceiling: DisclosureCeiling,
        _phase: HandleAuthorizationPhase,
    ) -> Result<Self::Checkpoint, HandleError> {
        Ok(TestCheckpoint)
    }
}

#[derive(Debug)]
struct TestObservation(EphemeralHandleTarget);

impl PackageOpaque for TestObservation {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

struct TestReadback {
    target: EphemeralHandleTarget,
    reads: usize,
    last_maximum_bytes: Option<u64>,
}

struct MonotonicFixture {
    store: HandleStore,
    handle: Result<SearchSourceHandle, HandleError>,
    crypto: TestCrypto,
    drops: Arc<AtomicUsize>,
}

fn monotonic_fixture(wrong_deadline: bool) -> MonotonicFixture {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut protection = TestProtection {
        available: true,
        wrong_deadline,
        drops: Arc::clone(&drops),
    };
    let mut crypto = TestCrypto { next: 0 };
    let mut store = HandleStore::new(HandlePolicy::BASELINE).unwrap();
    let (binding, grant, epoch, disclosure) = binding();
    let handle = store.mint_ephemeral_retained_source(
        target(),
        SourceMembershipId::from_bytes([9; 16]),
        RepresentationId::from_bytes([10; 16]),
        UnitId::from_bytes([11; 16]),
        binding,
        grant,
        epoch,
        disclosure,
        &mut protection,
        &mut crypto,
        false,
    );
    MonotonicFixture {
        store,
        handle,
        crypto,
        drops,
    }
}

fn retained_readback() -> TestReadback {
    TestReadback {
        target: EphemeralHandleTarget::RetainedSource(target()),
        reads: 0,
        last_maximum_bytes: None,
    }
}

#[test]
fn monotonic_expired_token_never_calls_readback_or_output() {
    let MonotonicFixture {
        mut store,
        handle,
        crypto,
        drops,
    } = monotonic_fixture(false);
    let handle = handle.unwrap();
    let stored = store.records.values_mut().next().unwrap();
    // Simulate elapsed process time after idle; canonical UTC is unchanged.
    stored.monotonic_deadline = Instant::now().checked_sub(Duration::from_secs(1));
    let mut readback = retained_readback();
    let mut delivered = false;
    let result = store.expand(
        &handle,
        4,
        &crypto,
        &mut TestAuthorization,
        &mut readback,
        |_| {
            delivered = true;
            Ok::<(), ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(HandleDeliveryError::Handle(HandleError::HandleExpired))
    ));
    assert_eq!(readback.reads, 0);
    assert!(!delivered);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let stored = store.records.values().next().unwrap();
    assert_eq!(
        stored.status(),
        search_contracts::LifecycleRecordStatus::Expired
    );
    assert!(stored.monotonic_deadline.is_none());
    assert!(stored.source_protection.is_none());
}

struct ExpireAtOutput {
    observed: Arc<Mutex<Instant>>,
    deadline: Instant,
}

impl std::fmt::Debug for ExpireAtOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ExpireAtOutput(<redacted>)")
    }
}

impl PackageOpaque for ExpireAtOutput {
    fn owner_package(&self) -> &'static str {
        "search-handles-test"
    }
}

impl HandleReauthorizationPort for ExpireAtOutput {
    type Checkpoint = TestCheckpoint;

    fn checkpoint(
        &mut self,
        _record: &SearchSourceHandleRecord,
        _membership: Option<SourceMembershipId>,
        _representation: Option<RepresentationId>,
        _unit: Option<UnitId>,
        _epoch: OwnerEpoch,
        _disclosure: DisclosureCeiling,
        phase: HandleAuthorizationPhase,
    ) -> Result<TestCheckpoint, HandleError> {
        if phase == HandleAuthorizationPhase::BeforeOutput {
            *self.observed.lock().unwrap() = self.deadline;
        }
        Ok(TestCheckpoint)
    }
}

#[test]
fn monotonic_expiry_before_output_releases_guard_without_delivery() {
    let MonotonicFixture {
        mut store,
        handle,
        crypto,
        drops,
    } = monotonic_fixture(false);
    let handle = handle.unwrap();
    let deadline = store
        .records
        .values()
        .next()
        .unwrap()
        .monotonic_deadline
        .unwrap();
    let observed = Arc::new(Mutex::new(Instant::now()));
    let mut authorization = ExpireAtOutput {
        observed: Arc::clone(&observed),
        deadline,
    };
    let mut readback = retained_readback();
    let mut delivered = false;
    // Only this owner-private test seam accepts observations; public expand uses Instant::now.
    let result = store.expand_with_clock(
        &handle,
        4,
        &crypto,
        &mut authorization,
        &mut readback,
        |_| {
            delivered = true;
            Ok::<(), ()>(())
        },
        || *observed.lock().unwrap(),
    );
    assert!(matches!(
        result,
        Err(HandleDeliveryError::Handle(HandleError::HandleExpired))
    ));
    assert_eq!(readback.reads, 1);
    assert!(!delivered);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(
        store
            .records
            .values()
            .next()
            .unwrap()
            .monotonic_deadline
            .is_none()
    );
}

#[test]
fn monotonic_expire_releases_deadline_and_guard_must_echo_exact_deadline() {
    let MonotonicFixture {
        mut store, drops, ..
    } = monotonic_fixture(false);
    store
        .records
        .values_mut()
        .next()
        .unwrap()
        .monotonic_deadline = Instant::now().checked_sub(Duration::from_secs(1));
    let expired = store.expire();
    assert_eq!(expired.expired, 1);
    assert!(!expired.more_expired);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(
        store
            .records
            .values()
            .next()
            .unwrap()
            .monotonic_deadline
            .is_none()
    );

    let mismatch = monotonic_fixture(true);
    assert!(matches!(
        mismatch.handle,
        Err(HandleError::SourceProtectionMismatch)
    ));
    assert!(mismatch.store.is_empty());
    assert_eq!(mismatch.drops.load(Ordering::SeqCst), 1);
}

impl HandleReadbackPort for TestReadback {
    type Error = ();
    type OwnerObservation = TestObservation;

    fn read_exact(
        &mut self,
        permit: &HandleExpansionPermit,
    ) -> Result<(HandleReadback, Self::OwnerObservation), Self::Error> {
        self.last_maximum_bytes = Some(permit.maximum_bytes());
        if self.target != *permit.target() || permit.maximum_bytes() < 4 {
            return Err(());
        }
        self.reads += 1;
        Ok((
            HandleReadback {
                target: self.target.clone(),
                bytes: b"body".to_vec(),
            },
            TestObservation(self.target.clone()),
        ))
    }

    fn verify_owner_observation(
        &self,
        observation: &Self::OwnerObservation,
        permit: &HandleExpansionPermit,
    ) -> Result<(), Self::Error> {
        if observation.0 == *permit.target() {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[test]
fn retained_target_range_token_and_invalidation_are_store_owned() {
    let retained_target = target();
    let membership = SourceMembershipId::from_bytes([9; 16]);
    let canonical_target = EphemeralHandleTarget::RetainedSource(retained_target.clone());
    let drops = Arc::new(AtomicUsize::new(0));
    let mut protection = TestProtection {
        available: false,
        wrong_deadline: false,
        drops: Arc::clone(&drops),
    };
    let mut crypto = TestCrypto { next: 0 };
    let wide_ceiling = 32 * 1_024 * 1_024;
    let policy = HandlePolicy {
        max_expansion_bytes: wide_ceiling,
        ..HandlePolicy::BASELINE
    };
    let mut store = HandleStore::new(policy).unwrap();
    let (binding_id, grant_id, owner_epoch, disclosure_ceiling) = binding();

    assert_eq!(
        store.mint_ephemeral_retained_source(
            retained_target.clone(),
            membership,
            RepresentationId::from_bytes([10; 16]),
            UnitId::from_bytes([11; 16]),
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut protection,
            &mut crypto,
            false,
        ),
        Err(HandleError::SourceProtectionUnavailable),
        "no source-owner guard means no retained-source handle"
    );
    assert!(store.is_empty());

    protection.available = true;
    let handle = store
        .mint_ephemeral_retained_source(
            retained_target.clone(),
            membership,
            RepresentationId::from_bytes([10; 16]),
            UnitId::from_bytes([11; 16]),
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut protection,
            &mut crypto,
            false,
        )
        .unwrap();
    let mut authorization = TestAuthorization;
    let mut readback = TestReadback {
        target: canonical_target,
        reads: 0,
        last_maximum_bytes: None,
    };
    let expected_excerpt = target_excerpt_digest(&readback.target);

    let too_small = store.expand(
        &handle,
        3,
        &crypto,
        &mut authorization,
        &mut readback,
        |_| Ok::<(), ()>(()),
    );
    assert!(matches!(
        too_small,
        Err(HandleDeliveryError::Handle(
            HandleError::RangeBudgetExceeded
        ))
    ));
    assert_eq!(
        readback.reads, 0,
        "range ceiling is checked before readback"
    );

    let delivered = store
        .expand(
            &handle,
            wide_ceiling,
            &crypto,
            &mut authorization,
            &mut readback,
            |expansion| {
                assert_eq!(expansion.bytes, b"body");
                assert_eq!(expansion.excerpt_digest, expected_excerpt);
                Ok::<_, ()>(expansion.handle_id)
            },
        )
        .unwrap();
    assert_eq!(delivered, handle.handle_id);
    assert_eq!(readback.reads, 1);
    assert_eq!(
        readback.last_maximum_bytes,
        Some(4),
        "a 32 MiB policy/request still reads only the four-byte original range"
    );

    let receipt = store
        .invalidate(&HandleInvalidationScope::Membership(membership), 1)
        .unwrap();
    assert_eq!(receipt.invalidated, 1);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "invalidation drops the held source guard"
    );
    assert!(matches!(
        store.expand(
            &handle,
            4,
            &crypto,
            &mut authorization,
            &mut readback,
            |_| Ok::<(), ()>(())
        ),
        Err(HandleDeliveryError::Handle(
            HandleError::HandleInvalidated | HandleError::HandleNotFound
        ))
    ));
    assert_eq!(readback.reads, 1, "invalidated token fails before readback");

    let mut oversized_target = retained_target;
    let oversized_length = u64::try_from(MAX_BLAKE3_EXACT_BYTES).unwrap() + 1;
    oversized_target.source_revision_ref.byte_length = oversized_length;
    oversized_target.native_anchor = NativeAnchor::TextBytes(TextBytesAnchor {
        content_digest: oversized_target.source_revision_ref.content_digest,
        byte_start_0: 0,
        byte_end_exclusive_0: oversized_length,
    });
    let mut oversized_store = HandleStore::new(policy).unwrap();
    let oversized_handle = oversized_store
        .mint_ephemeral_retained_source(
            oversized_target.clone(),
            membership,
            RepresentationId::from_bytes([10; 16]),
            UnitId::from_bytes([11; 16]),
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut protection,
            &mut crypto,
            false,
        )
        .unwrap();
    let mut oversized_readback = TestReadback {
        target: EphemeralHandleTarget::RetainedSource(oversized_target),
        reads: 0,
        last_maximum_bytes: None,
    };
    assert!(matches!(
        oversized_store.expand(
            &oversized_handle,
            wide_ceiling,
            &crypto,
            &mut authorization,
            &mut oversized_readback,
            |_| Ok::<(), ()>(())
        ),
        Err(HandleDeliveryError::Handle(
            HandleError::RangeBudgetExceeded
        ))
    ));
    assert_eq!(
        oversized_readback.last_maximum_bytes, None,
        "a selected range above the shared hash ceiling must fail before I/O"
    );

    let mut buffer_readback = TestReadback {
        target: buffer_target(14),
        reads: 0,
        last_maximum_bytes: None,
    };
    let buffer_handle = store
        .mint_ephemeral(
            buffer_readback.target.clone(),
            membership,
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut crypto,
            false,
        )
        .unwrap();
    let receipt = store
        .invalidate(&HandleInvalidationScope::Membership(membership), 2)
        .unwrap();
    assert_eq!(
        receipt.invalidated, 1,
        "exact membership invalidation includes buffers"
    );
    assert!(matches!(
        store.expand(
            &buffer_handle,
            4,
            &crypto,
            &mut authorization,
            &mut buffer_readback,
            |_| Ok::<(), ()>(())
        ),
        Err(HandleDeliveryError::Handle(
            HandleError::HandleInvalidated | HandleError::HandleNotFound
        ))
    ));

    let denied_handle = store
        .mint_ephemeral(
            buffer_target(14),
            membership,
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut crypto,
            false,
        )
        .unwrap();
    let purged_membership = SourceMembershipId::from_bytes([15; 16]);
    let purged_handle = store
        .mint_ephemeral(
            buffer_target(16),
            purged_membership,
            binding_id,
            grant_id,
            owner_epoch,
            disclosure_ceiling,
            &mut crypto,
            false,
        )
        .unwrap();
    let denied = BoundedSet::<_, MAX_SET_ITEMS>::from_items([membership]).unwrap();
    let purged = BoundedSet::<_, MAX_SET_ITEMS>::from_items([purged_membership]).unwrap();
    let mut pass = store.begin_security_invalidation(&denied, &purged, 3);
    assert!(pass.advance().unwrap().complete);
    let receipt = pass.finish().unwrap();
    assert_eq!(
        receipt.invalidated(),
        2,
        "security acknowledgement includes denied and purged buffers"
    );
    for handle in [denied_handle, purged_handle] {
        assert!(matches!(
            store.expand(
                &handle,
                4,
                &crypto,
                &mut authorization,
                &mut buffer_readback,
                |_| Ok::<(), ()>(())
            ),
            Err(HandleDeliveryError::Handle(
                HandleError::HandleInvalidated | HandleError::HandleNotFound
            ))
        ));
    }
    assert_eq!(
        buffer_readback.last_maximum_bytes, None,
        "revoked buffer tokens fail before readback"
    );
}
