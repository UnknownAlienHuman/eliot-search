//! Mandatory comparison-input fixtures. AUTHORED_NOT_RUN.
//!
//! Fixture additions for the mandatory `NativeClientExpectations` boundary.
//! Synthetic owner coordinates only: no native transport, no server
//! authorization proof, no endpoint or credential material.

#[cfg(test)]
mod fixtures {


    use search_contracts::{
        BindingId, Blake3Digest32, InstallationId, InstallationIncarnationId, OpaqueRef,
        ProtocolRange, ProtocolVersion,
    };

    use crate::typed::native::NativeClientExpectations;


    /// Exact 1.0 range accepted by the mandatory constructor.
    fn accepted_range() -> ProtocolRange {
        let version = ProtocolVersion { major: 1, minor: 0 };
        ProtocolRange::new(version, version).expect("synthetic 1.0 range")
    }

    /// Build one expectations value from synthetic, independently owned inputs.
    fn expectations(installation: u8, incarnation: u8, binding: u8) -> NativeClientExpectations {
        NativeClientExpectations::new(
            InstallationId::from_bytes([installation; 16]),
            InstallationIncarnationId::from_bytes([incarnation; 16]),
            BindingId::from_bytes([binding; 16]),
            OpaqueRef::new("synthetic-pairing-ref").expect("synthetic ref"),
            accepted_range(),
            Some(Blake3Digest32::from_bytes([0xAB; 32])),
        )
        .expect("synthetic expectations")
    }

    #[test]
    fn accessors_return_exactly_the_supplied_comparison_coordinates() {
        let expected = expectations(11, 22, 33);

        assert_eq!(
            expected.installation_id(),
            InstallationId::from_bytes([11; 16])
        );
        assert_eq!(
            expected.installation_incarnation_id(),
            InstallationIncarnationId::from_bytes([22; 16])
        );
        assert_eq!(expected.binding_id(), BindingId::from_bytes([33; 16]));
        assert_eq!(expected.supported_protocol_range(), accepted_range());
    }

    #[test]
    fn constructor_rejects_any_range_other_than_the_exact_one_zero_profile() {
        let version = ProtocolVersion { major: 1, minor: 0 };

        let narrowed = ProtocolRange {
            minimum: ProtocolVersion { major: 0, minor: 9 },
            maximum: version,
        };
        assert!(NativeClientExpectations::new(
            InstallationId::from_bytes([1; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            BindingId::from_bytes([3; 16]),
            OpaqueRef::new("synthetic-pairing-ref").expect("synthetic ref"),
            narrowed,
            Some(Blake3Digest32::from_bytes([0xAB; 32])),
        )
        .is_err());

        let shifted = ProtocolRange::new(
            ProtocolVersion { major: 1, minor: 1 },
            ProtocolVersion { major: 1, minor: 1 },
        )
        .expect("range constructs");
        assert!(NativeClientExpectations::new(
            InstallationId::from_bytes([1; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            BindingId::from_bytes([3; 16]),
            OpaqueRef::new("synthetic-pairing-ref").expect("synthetic ref"),
            shifted,
            Some(Blake3Digest32::from_bytes([0xAB; 32])),
        )
        .is_err());
    }

    #[test]
    fn debug_is_content_free_and_omits_identity_coordinates() {
        // Registration coordinates, descriptor text, endpoint names and any
        // credential-like material must not reach public diagnostics.
        let expected = expectations(11, 22, 33);
        let rendered = format!("{expected:?}");

        assert!(rendered.starts_with("NativeClientExpectations"), "{rendered}");
        assert!(rendered.contains("protocol"), "{rendered}");
        assert!(rendered.contains(".."), "{rendered}");

        for forbidden in [
            "11", "22", "33", "pairing-ref", "synthetic", "installation", "incarnation",
            "binding", "credential", "endpoint", "pipe",
        ] {
            assert!(!rendered.contains(forbidden), "{forbidden} leaked into {rendered}");
        }
    }

    #[test]
    fn rejected_expectation_error_path_stays_content_free() {
        // TypedClientError keeps reason and diagnostics closed, so a refused
        // expectation cannot smuggle path, credential or endpoint text out.
        let version = ProtocolVersion { major: 1, minor: 0 };
        let widened = ProtocolRange {
            minimum: version,
            maximum: ProtocolVersion { major: 1, minor: 1 },
        };
        let error = NativeClientExpectations::new(
            InstallationId::from_bytes([1; 16]),
            InstallationIncarnationId::from_bytes([2; 16]),
            BindingId::from_bytes([3; 16]),
            OpaqueRef::new("synthetic-pairing-ref").expect("synthetic ref"),
            widened,
            Some(Blake3Digest32::from_bytes([0xAB; 32])),
        )
        .expect_err("range outside the exact 1.0 profile must be refused");

        // Only the closed code is asserted; the message text is not fixed here
        // because the protocol error vocabulary is owned elsewhere.
        assert!(!error.code().is_empty());
        assert!(!error.to_string().contains("pipe"));
        assert!(!error.to_string().contains("synthetic-pairing-ref"));

    }


}
