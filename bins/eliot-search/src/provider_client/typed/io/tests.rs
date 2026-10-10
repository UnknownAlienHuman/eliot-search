//! Synthetic setup fixtures; no platform connector or native authentication proof.

#[cfg(test)]
mod fixtures {
    use std::cell::Cell;
    use std::io::{self, Read, Write};
    use std::time::{Duration, Instant};

    use search_contracts::{
        BindingId, InstallationId, InstallationIncarnationId, OpaqueRef, ProtocolRange,
        ProtocolVersion, RequestId,
    };
    use search_ports::{CancellationProbe, OperationContext, PackageOpaque};
    use search_provider_protocol::{BindingKey, DEFAULT_PROTOCOL_LIMITS, NativeEndpointNameV1};

    use super::super::{LocalByteStream, SetupBudget};
    use crate::provider_client::typed::{
        NativeClientBinding, TypedClientError, TypedProviderSession,
    };

    #[derive(Clone, Copy, Debug)]
    struct Cancel(bool);

    impl PackageOpaque for Cancel {
        fn owner_package(&self) -> &'static str {
            "eliot-search"
        }
    }

    impl CancellationProbe for Cancel {
        fn is_cancelled(&self) -> bool {
            self.0
        }
    }

    struct NeverStream;

    impl Read for NeverStream {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("unexpected read")
        }
    }

    impl Write for NeverStream {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("unexpected write")
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("unexpected flush")
        }
    }

    impl LocalByteStream for NeverStream {
        fn configure_local(&self) -> Result<(), TypedClientError> {
            panic!("unexpected configure")
        }
    }

    fn connect_attempt(
        deadline: Instant,
        cancelled: bool,
        calls: &Cell<usize>,
    ) -> Result<TypedProviderSession, TypedClientError> {
        let installation = InstallationId::from_bytes([1; 16]);
        let version = ProtocolVersion { major: 1, minor: 0 };
        let binding = NativeClientBinding::new_trusted(
            installation,
            InstallationIncarnationId::from_bytes([2; 16]),
            BindingId::from_bytes([3; 16]),
            OpaqueRef::new("pairing-ref").expect("ref"),
            ProtocolRange::new(version, version).expect("range"),
            None,
        )
        .expect("binding");
        let context = OperationContext::new(
            RequestId::from_bytes([4; 16]),
            60_000,
            Cancel(cancelled),
            OpaqueRef::new("budget").expect("budget"),
        )
        .expect("positive relative context");
        TypedProviderSession::connect_local(
            NativeEndpointNameV1::from_installation(installation),
            &binding,
            BindingKey::from_bytes([0xAB; 32]).expect("synthetic key"),
            DEFAULT_PROTOCOL_LIMITS,
            &context,
            deadline,
            |_, _| {
                calls.set(calls.get() + 1);
                Err::<NeverStream, _>(io::Error::other("connector sentinel"))
            },
        )
    }

    #[test]
    fn setup_budget_preserves_the_exact_supplied_instant() {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(3600))
            .expect("deadline");
        let cancelled = || false;
        let budget = SetupBudget::from_deadline(deadline, &cancelled).expect("budget");
        assert_eq!(budget.deadline, deadline);
        assert!(budget.remaining().expect("remaining") <= Duration::from_secs(3600));
    }

    #[test]
    fn expired_origin_refuses_before_connect_despite_positive_relative_context() {
        let deadline = Instant::now();
        let calls = Cell::new(0);
        assert!(matches!(
            connect_attempt(deadline, false, &calls),
            Err(TypedClientError::DeadlineExpired)
        ));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn cancellation_refuses_before_connect_with_unexpired_origin() {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(3600))
            .expect("deadline");
        let calls = Cell::new(0);
        assert!(matches!(
            connect_attempt(deadline, true, &calls),
            Err(TypedClientError::Cancelled)
        ));
        assert_eq!(calls.get(), 0);
    }
}
