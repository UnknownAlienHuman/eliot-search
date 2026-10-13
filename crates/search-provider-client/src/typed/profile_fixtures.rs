//! Typed transport profile acknowledgement fixtures. Fixture execution evidence is recorded at the accepted head.
//!
//! Synthetic owner fixtures for the post-pairing typed-profile negotiation on
//! one already-connected local stream. No platform connector, native
//! authentication proof or daemon authorization policy is exercised: these
//! cases assert only what the client's own profile owner accepts or refuses.
//! The server's capability decision stays outside this client boundary.

#[cfg(test)]
mod fixtures {
    use std::cell::RefCell;
    use std::io::{self, Read, Write};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use search_contracts::{
        BindingId, HelloBody, InstallationIncarnationId, OpaqueRef, ProtocolRange, ProtocolVersion,
        protocol::PeerRole,
    };
    use search_provider_protocol::{
        BindingContext, BindingKey, ClientNonce, PairingChallenge, PairingMachine, ProofDigest,
        ServerNonce, SessionId, TransportPeer, TypedTransportProfileV1, authenticate_binding,
    };

    use crate::TypedProviderSession;
    use crate::typed::TypedClientError;
    use crate::typed::io::{LocalByteStream, SetupBudget, SocketIo};

    /// Queues the peer's negotiation reply and then holds the stream open.
    struct ProfileStream {
        pending: Vec<u8>,
        open: bool,
        reflect_offer: bool,
        emitted: Rc<RefCell<Vec<u8>>>,
    }

    impl Read for ProfileStream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() && self.reflect_offer {
                let reflected_proof = {
                    let emitted = self.emitted.borrow();
                    let proof_start = emitted
                        .len()
                        .checked_sub(TypedTransportProfileV1::PROOF_BYTES)
                        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
                    emitted
                        .get(proof_start..)
                        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?
                        .to_vec()
                };
                self.pending
                    .extend_from_slice(TypedTransportProfileV1::PREFACE);
                self.pending.extend_from_slice(&reflected_proof);
                self.reflect_offer = false;
            }
            let count = bytes.len().min(self.pending.len());
            let moved: Vec<u8> = self.pending.drain(..count).collect();
            bytes[..count].copy_from_slice(&moved);
            if count > 0 {
                return Ok(count);
            }
            if self.open {
                // The reply is not complete; the peer keeps the stream open.
                return Err(io::ErrorKind::WouldBlock.into());
            }
            // The peer closed before sending the whole acknowledgement.
            Ok(0)
        }
    }

    impl Write for ProfileStream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.emitted.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl LocalByteStream for ProfileStream {
        fn configure_local(&self) -> Result<(), TypedClientError> {
            Ok(())
        }
    }

    fn synthetic_key() -> BindingKey {
        BindingKey::from_bytes([0xAB; 32]).expect("synthetic key")
    }

    /// Fully verified pairing, binding context and handoff inputs.
    struct Handoff {
        binding: BindingContext,
        ceremony: PairingMachine,
        nonce: ServerNonce,
        key: BindingKey,
    }

    fn handoff() -> Handoff {
        let version = ProtocolVersion { major: 1, minor: 0 };
        let proof = ProofDigest::from_bytes([9; 32]);
        let mut ceremony = PairingMachine::new(version, proof);
        ceremony
            .issue_challenge(
                SessionId::from_bytes([1; 16]).expect("session"),
                ClientNonce::from_bytes([2; 16]).expect("client nonce"),
                PairingChallenge::from_bytes([3; 32]).expect("challenge"),
            )
            .expect("challenge issued");
        ceremony
            .verify_client_proof(&proof, &proof)
            .expect("client verified");
        // The handoff owner recomputes the provider proof over the server
        // transcript with the real key, so the synthetic ceremony must use the
        // same keyed derivation instead of a raw constant.
        let server_transcript = ceremony.server_transcript().expect("server transcript");
        let provider_proof = synthetic_key().with_bytes(|bytes| {
            ProofDigest::from_bytes(*blake3::keyed_hash(bytes, server_transcript.as_bytes()).as_bytes())
        });
        ceremony
            .issue_provider_proof(provider_proof)
            .expect("provider proof");
        let peer = TransportPeer {
            role: PeerRole::StandaloneCli,
            incarnation: InstallationIncarnationId::from_bytes([3; 16]),
            binding: BindingId::from_bytes([4; 16]),
        };
        let hello = HelloBody {
            peer_role: peer.role,
            pairing_proof_ref: OpaqueRef::new("pairing-ref").expect("ref"),
            supported_protocol_range: ProtocolRange::new(version, version).expect("range"),
            requested_capability_digest: None,
        };
        let pairing = ceremony.clone().into_verified().expect("synthetic pairing");
        let binding = authenticate_binding(&hello, &pairing, &peer.incarnation, &peer)
            .expect("synthetic binding");
        Handoff {
            binding,
            ceremony,
            nonce: ServerNonce::from_bytes([0x44; 16]).expect("server nonce"),
            key: synthetic_key(),
        }
    }

    /// Runs the real profile owner against one synthetic server reply.
    ///
    /// The peer holds the stream open after the reply, so a complete reply can
    /// finish without the peer closing underneath it.
    fn open_with_stream(
        reply: Vec<u8>,
        open: bool,
        reflect_offer: bool,
    ) -> (
        Result<TypedProviderSession, TypedClientError>,
        Rc<RefCell<Vec<u8>>>,
    ) {
        let handoff = handoff();
        let emitted = Rc::new(RefCell::new(Vec::new()));
        let never = || false;
        let budget = SetupBudget::from_deadline(Instant::now() + Duration::from_secs(60), &never)
            .expect("synthetic setup budget");
        let result = TypedProviderSession::from_paired_socket(
            SocketIo::from_local_stream(ProfileStream {
                pending: reply,
                open,
                reflect_offer,
                emitted: Rc::clone(&emitted),
            }),
            handoff.binding,
            handoff.ceremony,
            handoff.key,
            handoff.nonce,
            search_provider_protocol::DEFAULT_PROTOCOL_LIMITS,
            &budget,
        );
        (result, emitted)
    }

    fn open(reply: Vec<u8>) -> Result<TypedProviderSession, TypedClientError> {
        open_with_stream(reply, true, false).0
    }

    /// Expected proofs used only to check the bytes the client actually wrote.
    fn expected_profile_domain_proofs() -> (ProofDigest, ProofDigest) {
        let handoff = handoff();
        let pairing = handoff.ceremony.into_verified().expect("pairing");
        let ceremony_id = pairing.session();
        let offer_transcript =
            TypedTransportProfileV1::offer_transcript(&ceremony_id, &handoff.nonce);
        let accept_transcript =
            TypedTransportProfileV1::accept_transcript(&ceremony_id, &handoff.nonce);
        let offer = handoff.key.with_bytes(|bytes| {
            let mut hasher = blake3::Hasher::new_keyed(bytes);
            for part in offer_transcript {
                hasher.update(part);
            }
            ProofDigest::from_bytes(*hasher.finalize().as_bytes())
        });
        let accept = handoff.key.with_bytes(|bytes| {
            let mut hasher = blake3::Hasher::new_keyed(bytes);
            for part in accept_transcript {
                hasher.update(part);
            }
            ProofDigest::from_bytes(*hasher.finalize().as_bytes())
        });
        (offer, accept)
    }

    /// Same owner path, but the peer closes without any reply bytes.
    fn open_closed() -> Result<TypedProviderSession, TypedClientError> {
        open_with_stream(Vec::new(), false, false).0
    }

    #[test]
    fn profile_reply_with_a_different_exact_preface_is_refused_as_a_mismatch() {
        let mut reply = TypedTransportProfileV1::PREFACE.to_vec();
        let last = reply.len() - 1;
        reply[last] = b'!';
        assert!(matches!(
            open(reply),
            Err(TypedClientError::ProfileMismatch)
        ));
    }

    #[test]
    fn reflected_client_offer_proof_cannot_satisfy_the_separate_accept_domain() {
        // The server must prove its own accept transcript; echoing the client's
        // offer proof fails closed and never activates the session.
        let (result, emitted) = open_with_stream(Vec::new(), true, true);
        let (offer, accept) = expected_profile_domain_proofs();
        let emitted = emitted.borrow();
        let preface_end = TypedTransportProfileV1::PREFACE.len();
        assert_eq!(
            emitted.get(..preface_end),
            Some(TypedTransportProfileV1::PREFACE),
            "client emitted the typed-profile preface"
        );
        let emitted_proof = emitted
            .get(preface_end..)
            .expect("captured client offer proof");
        assert_eq!(
            emitted_proof,
            offer.as_bytes(),
            "outgoing proof authenticates the offer transcript"
        );
        assert_ne!(
            emitted_proof,
            accept.as_bytes(),
            "outgoing proof must not use the server accept domain"
        );
        assert!(matches!(
            result,
            Err(TypedClientError::Protocol(
                search_provider_protocol::ProtocolError::AuthenticationFailed
            ))
        ));
    }

    #[test]
    fn profile_reply_closed_before_the_acknowledgement_refuses_before_activation() {
        // The peer closes before sending the profile acknowledgement, so the
        // read fails closed and the owner is dropped rather than activated.
        match open_closed() {
            Err(TypedClientError::PeerClosed) => {}
            Err(other) => panic!("unexpected refusal: {}", other.code()),
            Ok(_) => panic!("profile must not activate without an acknowledgement"),
        }
    }

    #[test]
    fn a_refused_profile_never_yields_a_session_for_the_mandatory_grant() {
        // The refused owner is dropped, so no grant command can be issued on it.
        let mut reply = TypedTransportProfileV1::PREFACE.to_vec();
        let last = reply.len() - 1;
        reply[last] = b'!';
        // The success type deliberately has no Debug, so no error-extraction
        // helper applies; extract the owned error by an explicit match.
        let Err(open_error) = open(reply) else {
            panic!("profile must be refused")
        };
        assert!(matches!(open_error, TypedClientError::ProfileMismatch));
        assert_eq!(
            open_error.code(),
            "REMOTE_TYPED_PROFILE_MISMATCH",
            "closed reason code without identity material"
        );
        assert!(matches!(
            open_error.outcome_class(),
            crate::OutcomeClass::Unknown
        ));
    }
}
