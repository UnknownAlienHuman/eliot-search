use super::*;
use search_ports::{CancellationProbe, PackageOpaque};

struct SharedProbe(Arc<AtomicBool>);

impl std::fmt::Debug for SharedProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedProbe([redacted])")
    }
}

impl PackageOpaque for SharedProbe {
    fn owner_package(&self) -> &'static str {
        "search-qdrant-bridge-tests"
    }
}

impl CancellationProbe for SharedProbe {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

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

#[test]
fn cloned_contexts_observe_later_cancellation_from_the_same_probe() {
    let signal = Arc::new(AtomicBool::new(false));
    let probe: Arc<dyn CancellationProbe + Send + Sync> =
        Arc::new(SharedProbe(Arc::clone(&signal)));
    let context = OpContext::with_cancellation_probe(Duration::from_secs(5), probe);
    let cloned = context.clone();

    assert_eq!(context.check(), Ok(()));
    assert_eq!(cloned.check(), Ok(()));
    signal.store(true, Ordering::SeqCst);
    assert_eq!(context.check(), Err(BridgeError::Cancelled));
    assert_eq!(cloned.check(), Err(BridgeError::Cancelled));
    assert!(!format!("{context:?}").contains("SharedProbe"));
}
