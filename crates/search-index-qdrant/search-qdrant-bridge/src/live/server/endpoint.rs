//! Loopback endpoint validation, ephemeral port reservation and client setup.

use std::time::Duration;

use qdrant_client::Qdrant;

use crate::live::FIXTURE_API_KEY;

use super::super::LiveError;

/// Loopback-only endpoint. Construction rejects anything that is not an
/// explicit loopback host before any socket is opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveEndpoint {
    host: String,
    http_port: u16,
    grpc_port: u16,
}

impl LiveEndpoint {
    /// Builds an endpoint pair on one host port, rejecting non-loopback hosts.
    pub fn grpc(host: &str, grpc_port: u16) -> Result<Self, LiveError> {
        Self::loopback(host, grpc_port, grpc_port)
    }

    /// Builds an endpoint pair with distinct HTTP/gRPC ports.
    pub fn loopback(host: &str, http_port: u16, grpc_port: u16) -> Result<Self, LiveError> {
        if http_port == 0 || grpc_port == 0 {
            return Err(LiveError::EndpointNotLoopback);
        }
        let normalized = match host
            .trim()
            .trim_matches(['[', ']'])
            .to_ascii_lowercase()
            .as_str()
        {
            "127.0.0.1" | "localhost" => "127.0.0.1".to_owned(),
            "::1" => "::1".to_owned(),
            _ => return Err(LiveError::EndpointNotLoopback),
        };
        Ok(Self {
            host: normalized,
            http_port,
            grpc_port,
        })
    }

    /// gRPC URL of the disposable server.
    #[must_use]
    pub fn grpc_url(&self) -> String {
        if self.host == "::1" {
            format!("http://[{}]:{}", self.host, self.grpc_port)
        } else {
            format!("http://{}:{}", self.host, self.grpc_port)
        }
    }

    /// Normalized explicit loopback host selected for this endpoint.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Reserved HTTP port (server config evidence).
    #[must_use]
    pub const fn http_port(&self) -> u16 {
        self.http_port
    }

    /// Reserved gRPC port.
    #[must_use]
    pub const fn grpc_port(&self) -> u16 {
        self.grpc_port
    }
}

/// Reserves two distinct free loopback ports from the OS.
pub fn free_loopback_ports() -> Result<(u16, u16), LiveError> {
    let http = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    let grpc = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .local_addr()
        .map_err(|_| LiveError::EndpointNotLoopback)?
        .port();
    if http == 0 || grpc == 0 || http == grpc {
        return Err(LiveError::EndpointNotLoopback);
    }
    Ok((http, grpc))
}

/// Builds the pinned client for one already-qualified loopback endpoint.
pub(in crate::live) fn connect(endpoint: &LiveEndpoint) -> Result<Qdrant, LiveError> {
    connect_with_api_key(endpoint, Some(FIXTURE_API_KEY))
}

/// Private fixture-only client constructor used to prove authentication is
/// required. Production connects use `QdrantApiKeyLease` in `real`.
pub(in crate::live) fn connect_with_api_key(
    endpoint: &LiveEndpoint,
    api_key: Option<&str>,
) -> Result<Qdrant, LiveError> {
    // The client's own compatibility check is warn-only and tolerates ±1
    // minor, so it is skipped: the bridge enforces the exact qualified pair in
    // `probe_server_identity` and `QualifiedGate::admit` instead.
    let builder = Qdrant::from_url(&endpoint.grpc_url());
    let builder = match api_key {
        Some(key) => builder.api_key(key),
        None => builder,
    };
    builder
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .skip_compatibility_check()
        .build()
        .map_err(|_| LiveError::TransportFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_accepts_only_explicit_loopback_hosts() {
        for host in ["127.0.0.1", "LOCALHOST", "[::1]"] {
            let endpoint =
                LiveEndpoint::loopback(host, 6333, 6334).expect("explicit loopback host");
            assert_eq!(endpoint.http_port(), 6333);
            assert_eq!(endpoint.grpc_port(), 6334);
        }
        assert_eq!(
            LiveEndpoint::loopback("localhost", 6333, 6334)
                .expect("localhost alias is normalized")
                .host(),
            "127.0.0.1"
        );
        assert_eq!(
            LiveEndpoint::grpc("10.0.0.1", 6334),
            Err(LiveError::EndpointNotLoopback)
        );
    }

    #[test]
    fn free_ports_are_nonzero_and_distinct() {
        let (http, grpc) = free_loopback_ports().expect("loopback ports");
        assert_ne!(http, 0);
        assert_ne!(grpc, 0);
        assert_ne!(http, grpc);
    }
}
