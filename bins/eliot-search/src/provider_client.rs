//! Canonical provider client facade.
//!
//! The previously monolithic transport remains byte-for-byte in `core.rs`.
//! This facade adds the indexed-query request mode without duplicating the
//! pairing, framing, proof or response-verification machinery.

mod core;

use std::net::SocketAddr;
use std::path::Path;

use search_provider_protocol::encode_indexed_query;

/// Canonical namespaced endpoint descriptor produced by the transport core.
pub type NamespacedEndpoint = core::NamespacedEndpoint;

/// Maps one provider/local failure code to the canonical CLI exit code.
#[must_use]
pub fn exit_for_error(code: &str) -> u8 {
    core::exit_for_error(code)
}

/// Reads the bounded loopback endpoint descriptor from one data root.
pub fn read_endpoint_descriptor(data_root: &Path) -> Result<NamespacedEndpoint, String> {
    core::read_endpoint_descriptor(data_root)
}

/// Reads and derives the bounded development pairing key.
pub fn read_shim_key(path: &Path) -> Result<[u8; 32], String> {
    core::read_shim_key(path)
}

/// One validated provider request before pairing/session sealing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsignedRequest {
    /// Sealed envelope: daemon health.
    Health,
    /// Sealed envelope: protocol and build version.
    Version,
    /// Sealed envelope: graceful shutdown.
    Shutdown,
    /// Local readiness/capability diagnostics.
    Status,
    /// Idempotent cancellation of one in-flight identity.
    Cancel {
        /// Raw 16-byte target identity.
        target: [u8; 16],
    },
    /// Validated source-backed recipe query.
    Query {
        /// Whether ASCII-insensitive matching was requested.
        ascii_insensitive: bool,
        /// Raw query bytes.
        query: Vec<u8>,
    },
    /// Validated indexed lexical query.
    IndexedQuery {
        /// Raw non-empty query bytes before canonical mode framing.
        query: Vec<u8>,
    },
    /// Validated admission target.
    Ingest {
        /// Raw target bytes.
        target: Vec<u8>,
    },
    /// Validated handle expansion.
    Expand {
        /// Opaque handle token bytes.
        handle: Vec<u8>,
        /// Inclusive byte range start.
        start: u64,
        /// Exclusive byte range end.
        end: u64,
    },
}

impl UnsignedRequest {
    /// Validates a source-backed query through the canonical core bounds.
    pub fn query(ascii_insensitive: bool, query: &[u8]) -> Result<Self, String> {
        core::UnsignedRequest::query(ascii_insensitive, query)?;
        Ok(Self::Query {
            ascii_insensitive,
            query: query.to_vec(),
        })
    }

    /// Validates an indexed query and its complete marked core payload.
    pub fn indexed_query(query: &[u8]) -> Result<Self, String> {
        let payload =
            encode_indexed_query(query).map_err(|error| error.code().to_owned())?;
        core::UnsignedRequest::query(false, &payload)?;
        Ok(Self::IndexedQuery {
            query: query.to_vec(),
        })
    }

    /// Validates an ingest target through the canonical core bounds.
    pub fn ingest(target: &[u8]) -> Result<Self, String> {
        core::UnsignedRequest::ingest(target)?;
        Ok(Self::Ingest {
            target: target.to_vec(),
        })
    }

    /// Validates a handle expansion through the canonical core bounds.
    pub fn expand(handle: &[u8], start: u64, end: u64) -> Result<Self, String> {
        core::UnsignedRequest::expand(handle, start, end)?;
        Ok(Self::Expand {
            handle: handle.to_vec(),
            start,
            end,
        })
    }

    /// Validates a cancellation target through the canonical core parser.
    pub fn cancel(target_hex: &str) -> Result<Self, String> {
        match core::UnsignedRequest::cancel(target_hex)? {
            core::UnsignedRequest::Cancel { target } => Ok(Self::Cancel { target }),
            _ => Err("REMOTE_CANCEL_TARGET_INVALID".to_owned()),
        }
    }

    fn to_core(&self) -> Result<core::UnsignedRequest, String> {
        match self {
            Self::Health => Ok(core::UnsignedRequest::Health),
            Self::Version => Ok(core::UnsignedRequest::Version),
            Self::Shutdown => Ok(core::UnsignedRequest::Shutdown),
            Self::Status => Ok(core::UnsignedRequest::Status),
            Self::Cancel { target } => Ok(core::UnsignedRequest::Cancel { target: *target }),
            Self::Query {
                ascii_insensitive,
                query,
            } => core::UnsignedRequest::query(*ascii_insensitive, query),
            Self::IndexedQuery { query } => {
                let payload =
                    encode_indexed_query(query).map_err(|error| error.code().to_owned())?;
                core::UnsignedRequest::query(false, &payload)
            }
            Self::Ingest { target } => core::UnsignedRequest::ingest(target),
            Self::Expand { handle, start, end } => {
                core::UnsignedRequest::expand(handle, *start, *end)
            }
        }
    }
}

/// Pairing-authenticated provider session using the canonical core transport.
pub struct ProviderSession {
    inner: core::ProviderSession,
}

/// Opens and negotiates one canonical provider session.
pub fn open_session(
    address: SocketAddr,
    key: [u8; 32],
) -> Result<ProviderSession, String> {
    core::open_session(address, key).map(|inner| ProviderSession { inner })
}

impl ProviderSession {
    /// Sends one validated request through the existing proof-verifying core.
    pub fn invoke(&mut self, request: &UnsignedRequest) -> Result<(), String> {
        let request = request.to_core()?;
        self.inner.invoke(&request)
    }
}

/// Renders the exact non-envelope line used by the canonical core.
#[cfg(test)]
#[must_use]
pub fn render_op_line(request: &UnsignedRequest) -> Option<String> {
    request
        .to_core()
        .ok()
        .and_then(|request| core::render_op_line(&request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_provider_protocol::{STRICT_QUERY_PREFIX, decode_indexed_query};

    #[test]
    fn facade_preserves_direct_requests_and_marks_indexed_queries() {
        let direct = UnsignedRequest::query(false, b"needle").expect("direct query");
        assert_eq!(
            render_op_line(&direct),
            Some(format!("op\tquery\t{}", core::hex_encode(b"s:needle")))
        );

        let indexed = UnsignedRequest::indexed_query(b"needle").expect("indexed query");
        let line = render_op_line(&indexed).expect("op line");
        let encoded = line
            .strip_prefix("op\tquery\t")
            .expect("canonical query operation");
        let bytes = decode_hex_for_test(encoded);
        assert!(bytes.starts_with(STRICT_QUERY_PREFIX));
        assert_eq!(decode_indexed_query(&bytes), Ok(Some(&b"needle"[..])));
        assert!(UnsignedRequest::indexed_query(b"").is_err());
    }

    fn decode_hex_for_test(text: &str) -> Vec<u8> {
        fn nibble(byte: u8) -> u8 {
            match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("non-hex test byte"),
            }
        }
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
            .collect()
    }
}
