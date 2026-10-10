//! Synthetic registration/key source; no Credential Manager or native transport proof.

#[cfg(test)]
mod fixtures {
    use std::time::{Duration, Instant};

    use search_contracts::{
        BindingId, Blake3Digest32, InstallationId, InstallationIncarnationId, NonZeroRevision,
        OpaqueRef, RequestId,
    };
    use search_ports::{CancellationProbe, OperationContext, PackageOpaque};
    use search_provider_protocol::BindingKey;

    use super::super::{
        NativeLocalRegistration, ResolveNativeLocalError, resolve_registered_local,
    };
    use crate::native_registered::NativePairingKeySource;

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

    #[derive(Default)]
    struct Keys {
        calls: usize,
        observed: Option<(Instant, Duration, Instant)>,
    }

    impl NativePairingKeySource for Keys {
        type Error = std::io::Error;

        fn load_key(
            &mut self,
            _locator: &[u8; 32],
            remaining: &mut dyn FnMut() -> Option<Duration>,
        ) -> Result<Option<BindingKey>, Self::Error> {
            self.calls += 1;
            let before = Instant::now();
            let left = remaining().expect("original resolver budget");
            let after = Instant::now();
            self.observed = Some((before, left, after));
            Ok(Some(
                BindingKey::from_bytes([0xAB; 32]).expect("synthetic key"),
            ))
        }
    }

    fn registration() -> NativeLocalRegistration {
        NativeLocalRegistration::new(
            InstallationId::from_bytes([1; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            BindingId::from_bytes([3; 16]),
            Blake3Digest32::from_bytes([4; 32]),
            NonZeroRevision::new(1).expect("generation"),
            OpaqueRef::new("pairing-ref").expect("ref"),
            None,
        )
    }

    fn context(cancelled: bool) -> OperationContext<Cancel> {
        OperationContext::new(
            RequestId::from_bytes([5; 16]),
            60_000,
            Cancel(cancelled),
            OpaqueRef::new("budget").expect("budget"),
        )
        .expect("context")
    }

    #[test]
    fn actual_resolver_handoff_retains_the_key_read_budget_origin() {
        let registration = registration();
        let context = context(false);
        let mut keys = Keys::default();
        let resolved =
            resolve_registered_local(&registration, &mut keys, &context).expect("resolved");
        let (name, binding, _key, narrowed, deadline) = resolved.into_parts();
        let (before, left, after) = keys.observed.expect("key read budget observation");
        // The original deadline lies between the two samples surrounding its
        // actual remaining-budget callback. Reconstructed relative metadata is
        // deliberately not used to derive these bounds.
        assert!(deadline >= before.checked_add(left).expect("lower bound"));
        assert!(deadline <= after.checked_add(left).expect("upper bound"));
        assert_eq!(keys.calls, 1);
        assert_eq!(name, registration.endpoint_name());
        assert_eq!(
            binding.installation_id(),
            InstallationId::from_bytes([1; 16])
        );
        assert_eq!(narrowed.request_id(), context.request_id());
        assert_eq!(narrowed.budget_ref(), context.budget_ref());
        assert!(narrowed.relative_deadline_ms() <= context.relative_deadline_ms());
    }

    #[test]
    fn cancelled_resolution_never_reads_credentials() {
        let mut keys = Keys::default();
        assert!(matches!(
            resolve_registered_local(&registration(), &mut keys, &context(true)),
            Err(ResolveNativeLocalError::Cancelled)
        ));
        assert_eq!(keys.calls, 0);
    }
}
