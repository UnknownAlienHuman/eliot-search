//! Length-before-allocation framing and bounded real Serde serialization.

use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Write};

use crate::{
    ClientFrame, DecodedUtf8Result, FRAME_PREFIX_BYTES, MAX_BODY_BYTES, MAX_CLIENT_FRAME_BYTES,
    MAX_FRAME_BYTES, MAX_ITEMS, ProtocolError, QueryOutcome, QuerySummary, RawExactResult,
    SearchItem, SessionClientFrame, SessionReply, StopReason, Terminal,
};

/// Incremental storage for exactly one length-prefixed frame.
///
/// Callers retain this value across cancelled I/O polls. A push consumes only
/// bytes belonging to this frame, so trailing or subsequent bytes remain visible.
pub struct FrameBuffer {
    prefix: [u8; FRAME_PREFIX_BYTES],
    prefix_used: usize,
    bytes: Vec<u8>,
    used: usize,
    maximum: usize,
}

impl FrameBuffer {
    /// Creates a single-frame buffer with a complete-frame ceiling.
    pub fn new(maximum: usize) -> Result<Self, ProtocolError> {
        if !(FRAME_PREFIX_BYTES + 1..=MAX_FRAME_BYTES).contains(&maximum) {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(Self {
            prefix: [0; FRAME_PREFIX_BYTES],
            prefix_used: 0,
            bytes: Vec::new(),
            used: 0,
            maximum,
        })
    }

    /// Copies the next fragment, rejecting its declared length before allocating.
    ///
    /// Returns the consumed prefix of input. Never consumes a second frame.
    pub fn push(&mut self, input: &[u8]) -> Result<usize, ProtocolError> {
        let mut consumed = 0;
        if self.prefix_used < FRAME_PREFIX_BYTES {
            let count = (FRAME_PREFIX_BYTES - self.prefix_used).min(input.len());
            self.prefix[self.prefix_used..self.prefix_used + count]
                .copy_from_slice(&input[..count]);
            self.prefix_used += count;
            consumed += count;
            if self.prefix_used != FRAME_PREFIX_BYTES {
                return Ok(consumed);
            }
            let body = usize::try_from(u32::from_le_bytes(self.prefix))
                .map_err(|_| ProtocolError::FrameTooLarge)?;
            let total = body
                .checked_add(FRAME_PREFIX_BYTES)
                .ok_or(ProtocolError::FrameTooLarge)?;
            if body == 0 {
                return Err(ProtocolError::InvalidFrame);
            }
            if total > self.maximum {
                return Err(ProtocolError::FrameTooLarge);
            }
            self.bytes
                .try_reserve_exact(total)
                .map_err(|_| ProtocolError::ResourceExhausted)?;
            self.bytes.resize(total, 0);
            self.bytes[..FRAME_PREFIX_BYTES].copy_from_slice(&self.prefix);
            self.used = FRAME_PREFIX_BYTES;
        }
        let count = (self.bytes.len() - self.used).min(input.len() - consumed);
        self.bytes[self.used..self.used + count]
            .copy_from_slice(&input[consumed..consumed + count]);
        self.used += count;
        Ok(consumed + count)
    }

    /// Whether no byte has yet been received.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.prefix_used == 0
    }

    /// Whether the declared frame has arrived in full.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        !self.bytes.is_empty() && self.used == self.bytes.len()
    }

    /// The complete prefix and JSON body, available only after full reception.
    #[must_use]
    pub fn frame(&self) -> Option<&[u8]> {
        self.is_complete().then_some(self.bytes.as_slice())
    }
}

/// Serializes one validated query or correlated cancellation before any I/O.
pub fn encode_client_frame(message: &ClientFrame) -> Result<Vec<u8>, ProtocolError> {
    message.validate()?;
    encode(message, MAX_CLIENT_FRAME_BYTES)
}

/// Decodes and validates one complete query/control frame.
pub fn decode_client_frame(frame: &[u8]) -> Result<ClientFrame, ProtocolError> {
    let message: ClientFrame = decode(frame, MAX_CLIENT_FRAME_BYTES)?;
    message.validate()?;
    Ok(message)
}

/// Encodes one typed session operation with the existing bounded frame codec.
pub fn encode_session_frame(message: &SessionClientFrame) -> Result<Vec<u8>, ProtocolError> {
    message.validate()?;
    encode(message, MAX_CLIENT_FRAME_BYTES)
}

/// Decodes one complete session data/control frame without a second transport.
pub fn decode_session_frame(frame: &[u8]) -> Result<SessionClientFrame, ProtocolError> {
    let message: SessionClientFrame = decode(frame, MAX_CLIENT_FRAME_BYTES)?;
    message.validate()?;
    Ok(message)
}

/// Encodes a complete session reply, refusing partial or over-limit payloads.
pub fn encode_session_reply(reply: &SessionReply) -> Result<Vec<u8>, ProtocolError> {
    reply.validate()?;
    encode(reply, MAX_FRAME_BYTES)
}

/// Decodes and validates one whole session reply. The caller checks its active request/window.
pub fn decode_session_reply(frame: &[u8]) -> Result<SessionReply, ProtocolError> {
    let reply: SessionReply = decode(frame, MAX_FRAME_BYTES)?;
    reply.validate()?;
    Ok(reply)
}

/// Serializes one complete terminal, refusing an oversized result.
///
/// Complete query results normally use `prepare_complete`, which selects a whole
/// rank prefix and explicitly reports `OutputLimit`.
pub fn encode_terminal_frame(terminal: &Terminal) -> Result<Vec<u8>, ProtocolError> {
    terminal.validate(u8::try_from(MAX_ITEMS).map_err(|_| ProtocolError::InvalidMessage)?)?;
    encode(terminal, MAX_FRAME_BYTES)
}

/// Decodes the whole terminal and validates it against the actual request limit.
pub fn decode_terminal_frame(frame: &[u8], requested_limit: u8) -> Result<Terminal, ProtocolError> {
    let terminal: Terminal = decode(frame, MAX_FRAME_BYTES)?;
    terminal.validate(requested_limit)?;
    Ok(terminal)
}

/// Encodes a fully accounted raw execution without clipping a byte range or gap.
/// The producer has already retained only its requested whole-item prefix.
pub fn prepare_raw_exact(
    request_id: u64,
    result: RawExactResult,
    summary: Option<QuerySummary>,
) -> Result<Vec<u8>, ProtocolError> {
    let mut terminal = Terminal {
        request_id,
        outcome: QueryOutcome::RawExact { result },
        summary,
    };
    fit_exact_source_info(&mut terminal)?;
    encode_terminal_frame(&terminal)
}

/// Encodes a whole retained UTF-8 execution without clipping text or identity ranges.
pub fn prepare_decoded_utf8(
    request_id: u64,
    result: DecodedUtf8Result,
    summary: Option<QuerySummary>,
) -> Result<Vec<u8>, ProtocolError> {
    let mut terminal = Terminal {
        request_id,
        outcome: QueryOutcome::DecodedUtf8 { result },
        summary,
    };
    fit_exact_source_info(&mut terminal)?;
    encode_terminal_frame(&terminal)
}

fn fit_exact_source_info(terminal: &mut Terminal) -> Result<(), ProtocolError> {
    let requested = match &terminal.outcome {
        QueryOutcome::RawExact { result } => {
            result.items.iter().any(|item| item.source_info().is_some())
        }
        QueryOutcome::DecodedUtf8 { result } => {
            result.items.iter().any(|item| item.source_info().is_some())
        }
        _ => false,
    };
    if !requested {
        return Ok(());
    }
    if fits(terminal)? {
        return Ok(());
    }
    // Optional locators must not erase valid Exact items or scan facts.
    match &mut terminal.outcome {
        QueryOutcome::RawExact { result } => {
            for item in &mut result.items {
                item.omit_source_info_for_output();
            }
        }
        QueryOutcome::DecodedUtf8 { result } => {
            for item in &mut result.items {
                item.omit_source_info_for_output();
            }
        }
        _ => {}
    }
    Ok(())
}

/// Produces a complete terminal with the largest whole rank prefix that fits.
///
/// The counting passes use the real Serde representation and stop at the body
/// ceiling. Only the final fitting frame is allocated; no 25 MiB JSON body or
/// shortened text is ever constructed. Every input item is validated before
/// selecting the output prefix.
pub fn prepare_complete(
    request_id: u64,
    generation: u64,
    items: Vec<SearchItem>,
    summary: Option<QuerySummary>,
) -> Result<Vec<u8>, ProtocolError> {
    let terminal = Terminal {
        request_id,
        outcome: QueryOutcome::Complete {
            generation,
            items,
            stop: StopReason::Complete,
        },
        summary,
    };
    terminal.validate(u8::try_from(MAX_ITEMS).map_err(|_| ProtocolError::InvalidMessage)?)?;
    if fits(&terminal)? {
        return encode(&terminal, MAX_FRAME_BYTES);
    }
    let QueryOutcome::Complete { items, .. } = &terminal.outcome else {
        return Err(ProtocolError::InvalidMessage);
    };
    let mut low = 0;
    let mut high = items.len();
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        let candidate = complete_wire(
            request_id,
            generation,
            &items[..middle],
            terminal.summary.as_ref(),
        );
        if fits(&candidate)? {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    encode(
        &complete_wire(
            request_id,
            generation,
            &items[..low],
            terminal.summary.as_ref(),
        ),
        MAX_FRAME_BYTES,
    )
}

#[derive(Serialize)]
struct CompleteWire<'a> {
    request_id: u64,
    outcome: CompleteOutcome<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a QuerySummary>,
}

#[derive(Serialize)]
struct CompleteOutcome<'a> {
    status: &'static str,
    generation: u64,
    items: &'a [SearchItem],
    stop: StopReason,
}

const fn complete_wire<'a>(
    request_id: u64,
    generation: u64,
    items: &'a [SearchItem],
    summary: Option<&'a QuerySummary>,
) -> CompleteWire<'a> {
    CompleteWire {
        request_id,
        outcome: CompleteOutcome {
            status: "complete",
            generation,
            items,
            stop: StopReason::OutputLimit,
        },
        summary,
    }
}

fn decode<T: DeserializeOwned>(frame: &[u8], maximum: usize) -> Result<T, ProtocolError> {
    if frame.len() > maximum {
        return Err(ProtocolError::FrameTooLarge);
    }
    let prefix: [u8; FRAME_PREFIX_BYTES] = frame
        .get(..FRAME_PREFIX_BYTES)
        .ok_or(ProtocolError::InvalidFrame)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidFrame)?;
    let body =
        usize::try_from(u32::from_le_bytes(prefix)).map_err(|_| ProtocolError::FrameTooLarge)?;
    if body == 0 || body.checked_add(FRAME_PREFIX_BYTES) != Some(frame.len()) {
        return Err(ProtocolError::InvalidFrame);
    }
    serde_json::from_slice(&frame[FRAME_PREFIX_BYTES..]).map_err(|_| ProtocolError::InvalidMessage)
}

fn encode(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>, ProtocolError> {
    let mut output = CappedWriter {
        bytes: Vec::new(),
        maximum,
        failure: None,
    };
    output
        .bytes
        .try_reserve(FRAME_PREFIX_BYTES)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    output.bytes.resize(FRAME_PREFIX_BYTES, 0);
    if serde_json::to_writer(&mut output, value).is_err() {
        return Err(output.failure.unwrap_or(ProtocolError::Serialization));
    }
    let body = u32::try_from(output.bytes.len() - FRAME_PREFIX_BYTES)
        .map_err(|_| ProtocolError::FrameTooLarge)?;
    output.bytes[..FRAME_PREFIX_BYTES].copy_from_slice(&body.to_le_bytes());
    Ok(output.bytes)
}

struct CappedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    failure: Option<ProtocolError>,
}

impl Write for CappedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.len() > self.maximum - self.bytes.len() {
            self.failure = Some(ProtocolError::FrameTooLarge);
            return Err(io::Error::other("frame limit"));
        }
        if self.bytes.try_reserve(input.len()).is_err() {
            self.failure = Some(ProtocolError::ResourceExhausted);
            return Err(io::Error::other("frame allocation"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct CountWriter {
    length: usize,
    exceeded: bool,
}

impl Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_BODY_BYTES - self.length {
            self.exceeded = true;
            return Err(io::Error::other("frame limit"));
        }
        self.length += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fits(value: &impl Serialize) -> Result<bool, ProtocolError> {
    let mut output = CountWriter {
        length: 0,
        exceeded: false,
    };
    match serde_json::to_writer(&mut output, value) {
        Ok(()) => Ok(true),
        Err(_) if output.exceeded => Ok(false),
        Err(_) => Err(ProtocolError::Serialization),
    }
}
