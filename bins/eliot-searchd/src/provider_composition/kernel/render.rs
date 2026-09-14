//! Bounded provider JSON line rendering.

use search_contracts::ProtocolVersion;
use search_provider_protocol::{ProtocolError, ServerNonce};

use super::capability::ProviderCapabilities;
use super::codec::hex_encode;
use super::spec::{
    MAX_BLOCKERS, MAX_RENDERED_LINE_BYTES, ProviderOperation, protocol_reason,
};

/// Renders the `provider_hello` JSON line: version, nonce, capabilities.
///
/// Bounded by [`MAX_RENDERED_LINE_BYTES`]; overlong output fails instead of
/// truncating, so capability loss is never silent.
pub fn render_hello(
    version: ProtocolVersion,
    nonce: &ServerNonce,
    capabilities: &ProviderCapabilities,
    reconnect_cancelled: usize,
) -> Result<String, &'static str> {
    let line = format!(
        concat!(
            "{{\"event\":\"provider_hello\",\"version\":\"{}.{}\",",
            "\"nonce\":\"{}\",\"reconnect_cancelled\":{},",
            "\"capabilities\":{{\"health\":{},\"status\":{},",
            "\"version\":{},\"shutdown\":{},\"cancel\":{},",
            "\"ingest\":{},\"query\":{},\"expand\":{},",
            "\"indexed\":{}}},\"blockers\":[{}]}}"
        ),
        version.major,
        version.minor,
        hex_encode(nonce.as_bytes()),
        reconnect_cancelled,
        capabilities.health_available,
        capabilities.status_available,
        capabilities.version_available,
        capabilities.shutdown_available,
        capabilities.cancel_available,
        capabilities.ingest_available,
        capabilities.query_available,
        capabilities.expand_available,
        capabilities.indexed_available,
        capabilities
            .blockers
            .iter()
            .map(|code| format!("\"{code}\""))
            .collect::<Vec<_>>()
            .join(","),
    );
    if line.len() > MAX_RENDERED_LINE_BYTES {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    Ok(line)
}

/// Outcome class for `provider_op` response lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpStatus {
    /// Operation completed its declared work.
    Ok,
    /// Operation is validated but unavailable (explicit blockers attached).
    Unavailable,
    /// Operation failed before a verified success postcondition.
    Failed,
    /// Cancellation released a live in-flight identity.
    Cancelled,
    /// Cancellation found no live identity (idempotent outcome).
    UnknownOrTerminal,
}

impl OpStatus {
    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::UnknownOrTerminal => "unknown_or_terminal",
        }
    }
}

/// Renders one `provider_op` JSON line with a typed reason and blockers.
///
/// Unavailable outcomes always carry their blockers; the line is never an
/// empty success.
pub fn render_op_response(
    operation: ProviderOperation,
    status: OpStatus,
    reason: &str,
    blockers: &[&str],
) -> Result<String, &'static str> {
    if blockers.len() > MAX_BLOCKERS || reason.len() > 256 {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    let line = format!(
        "{{\"event\":\"provider_op\",\"op\":\"{}\",\"status\":\"{}\",\"reason\":{},\"blockers\":[{}]}}",
        operation.as_str(),
        status.as_str(),
        json_string(reason),
        blockers
            .iter()
            .map(|code| json_string(code))
            .collect::<Vec<_>>()
            .join(","),
    );
    if line.len() > MAX_RENDERED_LINE_BYTES {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    Ok(line)
}

/// Renders one `provider_error` JSON line for failures without a request ID.
pub fn render_provider_error(reason: &str) -> String {
    format!(
        "{{\"event\":\"provider_error\",\"reason\":{}}}",
        json_string(&reason.chars().take(256).collect::<String>())
    )
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}
