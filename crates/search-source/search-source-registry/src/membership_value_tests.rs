//! Membership value replacement must be fully checked before durable dispatch.

use super::*;
use crate::membership::{
    MembershipLifecycle, MembershipRecord, derive_membership_id, transition_membership,
};

#[test]
fn membership_revision_exhaustion_preserves_state_without_journal_write() {
    let key = bind_request("one", "corpus:test").key;
    let maximum = NonZeroRevision::new(u64::MAX).expect("maximum revision");
    let record = MembershipRecord::new(
        key.clone(),
        derive_membership_id(&key),
        NonZeroRevision::new(1).expect("generation"),
        maximum,
        MembershipLifecycle::Active,
        7,
        receipt("receipt:previous"),
    );
    let mut memberships = std::collections::BTreeMap::from([(key.clone(), record.clone())]);
    let mut port = journal();
    let ctx = context(cancel());
    assert_eq!(
        transition_membership(
            &mut memberships,
            &key,
            MembershipCommand::Restrict,
            maximum,
            7,
            8,
            &opaque("operation:exhausted-membership"),
            digest(83),
            &receipt("receipt:exhausted-membership"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::ContractExhausted)
    );
    assert_eq!(memberships.get(&key), Some(&record));
    assert!(port.is_empty(), "refusal must precede durable dispatch");
}
