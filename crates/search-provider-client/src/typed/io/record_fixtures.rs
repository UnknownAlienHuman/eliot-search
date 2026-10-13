//! Typed record framing fixtures. Fixture execution evidence is recorded at the accepted head.
//!
//! Synthetic owner fixtures for the partial, oversized and truncated record
//! paths of one connected byte stream. No platform connector, native
//! authentication proof or server authorization policy is exercised here:
//! these cases assert only what the client's own framing owner accepts or
//! refuses. The daemon's authorization decision stays outside this boundary.

#[cfg(test)]
mod fixtures {
    use std::io::{self, Read, Write};
    use std::task::Poll;
    use std::time::{Duration, Instant};

    use search_provider_protocol::{
        DEFAULT_PROTOCOL_LIMITS, FRAME_PREFIX_BYTES, ProtocolError, ProtocolLimits,
    };

    use crate::typed::TypedClientError;
    use crate::typed::io::SocketIo;

    /// A peer that queues exact bytes and then stays connected.
    ///
    /// Once the queued bytes are exhausted the stream reports a retryable
    /// condition instead of EOF, so a record that merely has not arrived yet is
    /// distinguishable from one the peer closed mid-frame.
    struct QueuedStream {
        pending: Vec<u8>,
        open: bool,
    }

    impl Read for QueuedStream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let count = bytes.len().min(self.pending.len());
            let moved: Vec<u8> = self.pending.drain(..count).collect();
            bytes[..count].copy_from_slice(&moved);
            if count > 0 {
                return Ok(count);
            }
            if self.open {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            Ok(0)
        }
    }

    impl Write for QueuedStream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl crate::typed::io::LocalByteStream for QueuedStream {
        fn configure_local(&self) -> Result<(), TypedClientError> {
            Ok(())
        }
    }

    /// An incomplete body under a well-formed prefix, then clean EOF.
    fn truncated_body() -> QueuedStream {
        let mut declared = 64_u32.to_le_bytes().to_vec();
        declared.extend_from_slice(&[0xAB; 16]);
        QueuedStream {
            pending: declared,
            open: false,
        }
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }

    /// Bounded conservative ceilings that still pass the honest floor check.
    fn small_limits() -> ProtocolLimits {
        ProtocolLimits {
            max_frame_bytes: 4_096,
            max_body_bytes: 4_096 - FRAME_PREFIX_BYTES,
            ..DEFAULT_PROTOCOL_LIMITS
        }
    }

    #[test]
    fn truncated_record_body_is_refused_as_peer_closed_without_releasing_a_proof() {
        let mut socket = SocketIo::from_local_stream(truncated_body());
        let first =
            socket.poll_record(DEFAULT_PROTOCOL_LIMITS, deadline(), Duration::from_millis(50));
        assert!(matches!(first, Err(TypedClientError::PeerClosed)));
        // No partial proof is ever returned to the caller from an incomplete
        // record, and the retained partial record stays refused afterwards.
        assert!(matches!(
            socket.poll_record(
                DEFAULT_PROTOCOL_LIMITS,
                deadline(),
                Duration::from_millis(50)
            ),
            Err(TypedClientError::PeerClosed)
        ));
    }

    #[test]
    fn announced_length_above_the_ceiling_fails_before_any_body_is_reserved() {
        // The prefix is complete, but the announced length alone exceeds the
        // configured maximum body size, so no body may be reserved.
        let limits = small_limits();
        // The prefix bound is a u32 on the wire, so convert and check rather
        // than silently truncating a wider host value.
        let declared = (u32::try_from(limits.max_body_bytes).expect("fits the wire prefix") + 1)
            .to_le_bytes()
            .to_vec();
        let mut socket = SocketIo::from_local_stream(QueuedStream {
            pending: declared,
            open: true,
        });
        assert!(matches!(
            socket.poll_record(limits, deadline(), Duration::from_millis(50)),
            Err(TypedClientError::Protocol(ProtocolError::FrameTooLarge))
        ));
        // A refused length is not re-interpreted on a later poll of the same
        // retained record; the same refusal holds.
        assert!(matches!(
            socket.poll_record(limits, deadline(), Duration::from_millis(50)),
            Err(TypedClientError::Protocol(ProtocolError::FrameTooLarge))
        ));
    }

    #[test]
    fn partial_prefix_on_an_open_stream_stays_pending_within_the_quantum() {
        // Fewer than the four prefix bytes exist and the peer holds the
        // connection open, so no complete record may be fabricated.
        let mut socket = SocketIo::from_local_stream(QueuedStream {
            pending: vec![0x01, 0x00],
            open: true,
        });
        let quantum = Duration::from_millis(20);
        assert!(matches!(
            socket.poll_record(DEFAULT_PROTOCOL_LIMITS, deadline(), quantum),
            Ok(Poll::Pending)
        ));
        // Polling again on the same open stream still yields no record.
        assert!(matches!(
            socket.poll_record(DEFAULT_PROTOCOL_LIMITS, deadline(), quantum),
            Ok(Poll::Pending)
        ));
    }

    #[test]
    fn zero_quantum_is_refused_before_any_read_occurs() {
        let mut socket = SocketIo::from_local_stream(QueuedStream {
            pending: Vec::new(),
            open: true,
        });
        assert!(matches!(
            socket.poll_record(DEFAULT_PROTOCOL_LIMITS, deadline(), Duration::ZERO),
            Err(TypedClientError::Protocol(ProtocolError::InvalidLimits))
        ));
    }

    #[test]
    fn expired_deadline_stops_a_partial_record_without_finishing_it() {
        let mut socket = SocketIo::from_local_stream(truncated_body());
        assert!(matches!(
            socket.poll_record(
                DEFAULT_PROTOCOL_LIMITS,
                Instant::now(),
                Duration::from_millis(50)
            ),
            Err(TypedClientError::DeadlineExpired)
        ));
        // A later call must not convert the retained partial record into a
        // finished frame once its own deadline has passed.
        assert!(matches!(
            socket.poll_record(
                DEFAULT_PROTOCOL_LIMITS,
                deadline(),
                Duration::from_millis(50)
            ),
            Err(TypedClientError::DeadlineExpired | TypedClientError::PeerClosed)
        ));
    }
}
