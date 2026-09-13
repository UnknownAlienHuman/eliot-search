use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

use super::{BridgeError, OpContext, check_acknowledgement};

#[test]
fn completed_acknowledgement_allows_exact_readback() {
    assert_eq!(check_acknowledgement(true, &OpContext::default()), Ok(()));
}

#[test]
fn incomplete_acknowledgement_remains_unknown() {
    assert_eq!(
        check_acknowledgement(false, &OpContext::default()),
        Err(BridgeError::MutationOutcomeUnknown)
    );
}

#[test]
fn cancellation_after_dispatch_never_claims_no_write() {
    let flag = Arc::new(AtomicBool::new(false));
    let context = OpContext::with_cancel(Duration::from_secs(10), Arc::clone(&flag));
    assert_eq!(context.check(), Ok(()));
    flag.store(true, Ordering::SeqCst);
    assert_eq!(
        check_acknowledgement(true, &context),
        Err(BridgeError::MutationOutcomeUnknown)
    );
    // Before dispatch the same flag is still a definite cancellation.
    assert_eq!(context.check(), Err(BridgeError::Cancelled));
}
