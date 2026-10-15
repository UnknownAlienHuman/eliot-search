//! Independent frozen tuple/owner-codec binding, not Native custody qualification.
use super::*;
use search_contracts::{CanonicalValue, RequestId};
use search_control_redb::JournalIdentity;

fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([0x11; 16]),
        data_root_id: DataRootId::from_bytes([0x22; 16]),
        owner_epoch: OwnerEpoch::new(0x0304_0506_0708_090a).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([0x33; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([0x44; 32]),
        schema_version: 0x0506_0708,
    }
}

#[test]
fn direct_owner_profile_initialization_binds_exact_tuple_and_every_coordinate() {
    let identity = identity();
    let request = RequestId::from_bytes([0x91; 16]);
    let digest = [0xa2; 32];
    let operation =
        profile_initialization_operation(request, digest, identity, &|_| Ok(())).unwrap();
    // Exact pre-existing 108-byte owner codec, independently transcribed.
    let mut owner_bytes = vec![0x11; 16];
    owner_bytes.extend([0x22; 16]);
    owner_bytes.extend([3, 4, 5, 6, 7, 8, 9, 10]);
    owner_bytes.extend([0x33; 32]);
    owner_bytes.extend([0x44; 32]);
    owner_bytes.extend([5, 6, 7, 8]);
    let CanonicalValue::Bytes(actual) = identity.canonical_binding_value().unwrap() else {
        panic!("complete owner-codec byte value");
    };
    assert_eq!(actual.as_slice(), owner_bytes);
    // RFC 8949 tuple [1, bytes(16), bytes(32), bytes(108)], including framing.
    let mut preimage = b"eliot/cbor/control/direct-profile-initialization/v1\0".to_vec();
    preimage.extend([0x84, 0x01, 0x50]);
    preimage.extend([0x91; 16]);
    preimage.extend([0x58, 0x20]);
    preimage.extend([0xa2; 32]);
    preimage.extend([0x58, 0x6c]);
    preimage.extend(owner_bytes);
    assert!(preimage.len() <= 256);
    assert_eq!(operation.0, *blake3::hash(&preimage).as_bytes());

    let mut other_request = [0x91; 16];
    other_request[15] ^= 1;
    let mut other_digest = digest;
    other_digest[31] ^= 1;
    for (request, digest, other) in [
        (RequestId::from_bytes(other_request), digest, identity),
        (request, other_digest, identity),
        (
            request,
            digest,
            JournalIdentity {
                installation_incarnation_id: InstallationIncarnationId::from_bytes([0x12; 16]),
                ..identity
            },
        ),
        (
            request,
            digest,
            JournalIdentity {
                data_root_id: DataRootId::from_bytes([0x23; 16]),
                ..identity
            },
        ),
        (
            request,
            digest,
            JournalIdentity {
                owner_epoch: OwnerEpoch::new(identity.owner_epoch.get() + 1).unwrap(),
                ..identity
            },
        ),
        (
            request,
            digest,
            JournalIdentity {
                path_identity_digest: Blake3Digest32::from_bytes([0x34; 32]),
                ..identity
            },
        ),
        (
            request,
            digest,
            JournalIdentity {
                schema_family_digest: Blake3Digest32::from_bytes([0x45; 32]),
                ..identity
            },
        ),
        (
            request,
            digest,
            JournalIdentity {
                schema_version: identity.schema_version + 1,
                ..identity
            },
        ),
    ] {
        assert_ne!(
            operation,
            profile_initialization_operation(request, digest, other, &|_| Ok(())).unwrap()
        );
    }
}

#[test]
fn direct_owner_profile_initialization_precharges_and_preserves_first_refusal() {
    let request = RequestId::from_bytes([0x91; 16]);
    let attempts = RefCell::new(Vec::new());
    let reserve = |work| {
        attempts.borrow_mut().push(work);
        Ok(())
    };
    profile_initialization_operation(request, [0xa2; 32], identity(), &reserve).unwrap();
    let expected = vec![
        W::Allocate(256 + 4 * std::mem::size_of::<CanonicalValue>()),
        W::Process(256),
    ];
    assert_eq!(*attempts.borrow(), expected);
    for stop in 1..=2 {
        attempts.borrow_mut().clear();
        let reserve = |work| {
            attempts.borrow_mut().push(work);
            if attempts.borrow().len() == stop {
                Err(OwnerError::OwnerGuardMismatch)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            profile_initialization_operation(request, [0xa2; 32], identity(), &reserve),
            Err(OwnerError::OwnerGuardMismatch)
        ));
        assert_eq!(*attempts.borrow(), expected[..stop]);
    }
}
