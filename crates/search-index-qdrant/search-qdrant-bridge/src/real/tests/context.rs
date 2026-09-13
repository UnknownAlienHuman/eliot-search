use super::*;

#[test]
fn cancelled_context_fails_before_dispatch() {
    let flag = Arc::new(AtomicBool::new(true));
    let context =
        OpContext::with_cancel(Duration::from_secs(5), Arc::clone(&flag));
    assert_eq!(
        context.check().expect_err("cancelled"),
        BridgeError::Cancelled
    );
    flag.store(false, Ordering::SeqCst);
    assert!(context.check().is_ok());
    assert_eq!(OpContext::default().deadline(), Duration::from_secs(10));
}
