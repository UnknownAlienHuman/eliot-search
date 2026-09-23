//! Closed control-line grammar for the negotiated provider 1.0 transport.
//!
//! These parsers consume complete event-first control frames, not substrings
//! from source payloads. They are deliberately not a general JSON parser.
//! New control fields or spellings require explicit protocol negotiation.

use search_contracts::ProtocolVersion;
use search_provider_protocol::pairing::ServerNonce;

pub(super) fn invalid() -> String {
    "REMOTE_RESPONSE_INVALID".to_owned()
}

pub(super) fn authenticated(line: &str) -> Result<(), String> {
    if line == concat!(
        "{\"event\":\"authenticated\",\"protocol_version\":1,",
        "\"transport\":\"loopback_tcp\",\"authentication\":\"pairing_blake3_v1\"}"
    ) {
        Ok(())
    } else {
        Err("REMOTE_AUTHENTICATION_FAILED".to_owned())
    }
}

/// Classifies only the first top-level header. Payload JSON remains payload;
/// its nested strings cannot finish a request or declare a provider outcome.
pub(super) fn event(line: &str) -> Result<&str, String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":")?;
    let name = input.text()?;
    if name.is_empty()
        || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        || !(input.0 == "}" || (input.0.starts_with(',') && line.ends_with('}')))
    {
        return Err(invalid());
    }
    Ok(name)
}

pub(super) fn started(line: &str, expected: u64) -> Result<(), String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":\"request_started\",\"sequence\":")?;
    input.sequence(expected)?;
    input.literal("}")?;
    input.end()
}

/// None is a complete successful transport acknowledgement, not an operation
/// result. The caller must separately require its exact sealed/op outcome.
pub(super) fn complete(line: &str, expected: u64) -> Result<Option<&str>, String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":\"request_complete\",\"sequence\":")?;
    input.sequence(expected)?;
    input.literal(",\"ok\":")?;
    let error = if input.boolean()? {
        None
    } else {
        input.literal(",\"error\":")?;
        Some(input.code()?)
    };
    input.literal("}")?;
    input.end()?;
    Ok(error)
}

pub(super) fn hello(line: &str) -> Result<(ProtocolVersion, ServerNonce), String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":\"provider_hello\",\"version\":")?;
    let text = input.text()?;
    let (major, minor) = text.split_once('.').ok_or_else(invalid)?;
    let version = ProtocolVersion {
        major: major.parse().map_err(|_| invalid())?,
        minor: minor.parse().map_err(|_| invalid())?,
    };
    if text != format!("{}.{}", version.major, version.minor) {
        return Err(invalid());
    }
    input.literal(",\"nonce\":")?;
    let nonce = input.text()?;
    if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(invalid());
    }
    let nonce = ServerNonce::from_bytes(super::decode_16(nonce).map_err(|_| invalid())?)
        .map_err(|_| invalid())?;
    input.literal(",\"reconnect_cancelled\":")?;
    input.number()?;
    input.literal(",\"capabilities\":{")?;
    for (index, field) in [
        "health", "status", "version", "shutdown", "cancel", "ingest", "query", "expand", "indexed",
    ].into_iter().enumerate() {
        if index != 0 { input.literal(",")?; }
        if input.text()? != field { return Err(invalid()); }
        input.literal(":")?;
        input.boolean()?;
    }
    input.literal("},\"blockers\":")?;
    input.blockers()?;
    input.literal("}")?;
    input.end()?;
    Ok((version, nonce))
}

pub(super) fn provider_error(line: &str) -> Result<&str, String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":\"provider_error\",\"reason\":")?;
    let reason = input.code()?;
    input.literal("}")?;
    input.end()?;
    Ok(reason)
}

pub(super) struct OperationReply<'a> {
    status: &'a str,
    reason: &'a str,
}

impl OperationReply<'_> {
    /// A transport refusal must not be masked by an earlier `status:ok`.
    pub(super) fn acknowledge(&self, error: Option<&str>) -> Result<(), String> {
        let expected = if matches!(self.status, "failed" | "unavailable") {
            Some(self.reason)
        } else {
            None
        };
        if error == expected { Ok(()) } else { Err("REMOTE_RESPONSE_MISMATCH".to_owned()) }
    }

    pub(super) fn result(&self) -> Result<(), String> {
        match self.status {
            "ok" | "cancelled" => Ok(()),
            _ => Err(self.reason.to_owned()),
        }
    }
}

pub(super) fn operation<'a>(line: &'a str, expected: &str) -> Result<OperationReply<'a>, String> {
    let mut input = Cursor(line);
    input.literal("{\"event\":\"provider_op\",\"op\":")?;
    if input.text()? != expected { return Err("REMOTE_RESPONSE_MISMATCH".to_owned()); }
    input.literal(",\"status\":")?;
    let status = input.text()?;
    input.literal(",\"reason\":")?;
    let reason = input.code()?;
    input.literal(",\"blockers\":")?;
    let blockers = input.blockers()?;
    input.literal("}")?;
    input.end()?;
    match status {
        "ok" if reason == "PROVIDER_OK" && blockers == 0 => {}
        "cancelled" if expected == "cancel" && reason == "PROVIDER_OK" && blockers == 0 => {}
        "unknown_or_terminal" if expected == "cancel"
            && reason == "PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL" && blockers == 0 => {}
        "failed" | "unavailable" if reason != "PROVIDER_OK" => {}
        _ => return Err(invalid()),
    }
    Ok(OperationReply { status, reason })
}

struct Cursor<'a>(&'a str);

impl<'a> Cursor<'a> {
    fn literal(&mut self, text: &str) -> Result<(), String> {
        self.0 = self.0.strip_prefix(text).ok_or_else(invalid)?;
        Ok(())
    }

    // Control vocabulary is bounded unescaped ASCII. Never echo arbitrary
    // remote text as an error code, and never borrow through an escaped quote.
    fn text(&mut self) -> Result<&'a str, String> {
        self.literal("\"")?;
        let end = self.0.find('"').ok_or_else(invalid)?;
        let text = &self.0[..end];
        if text.len() > 256 || !text.bytes().all(|b| (b' '..=b'~').contains(&b) && b != b'\\') {
            return Err(invalid());
        }
        self.0 = &self.0[end + 1..];
        Ok(text)
    }

    fn code(&mut self) -> Result<&'a str, String> {
        let code = self.text()?;
        if code.is_empty() || !code.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') {
            return Err(invalid());
        }
        Ok(code)
    }

    fn number(&mut self) -> Result<u64, String> {
        let end = self.0.bytes().take_while(u8::is_ascii_digit).count();
        let text = &self.0[..end];
        if text.is_empty() || (text.len() > 1 && text.starts_with('0')) {
            return Err(invalid());
        }
        let number = text.parse().map_err(|_| invalid())?;
        self.0 = &self.0[end..];
        Ok(number)
    }

    fn sequence(&mut self, expected: u64) -> Result<(), String> {
        if self.number()? == expected { Ok(()) } else { Err("REMOTE_SEQUENCE_MISMATCH".to_owned()) }
    }

    fn boolean(&mut self) -> Result<bool, String> {
        if let Some(rest) = self.0.strip_prefix("true") {
            self.0 = rest;
            Ok(true)
        } else {
            self.literal("false")?;
            Ok(false)
        }
    }

    fn blockers(&mut self) -> Result<usize, String> {
        self.literal("[")?;
        let mut count = 0;
        if !self.0.starts_with(']') {
            loop {
                if count == 8 { return Err(invalid()); }
                self.code()?;
                count += 1;
                if !self.0.starts_with(',') { break; }
                self.literal(",")?;
            }
        }
        self.literal("]")?;
        Ok(count)
    }

    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err(invalid()) }
    }
}
