//! Fixed-order schema I/O. General JSON syntax is checked by FrameCodec first.

use search_contracts::{MAX_JSON_DEPTH, base64url_decode, base64url_encode};

use crate::error::ProtocolError;

pub(super) type Result<T> = core::result::Result<T, ProtocolError>;

pub(super) trait Schema: Sized {
    fn put(&self, output: &mut Encoder) -> Result<()>;
    fn get(input: &mut Decoder<'_>) -> Result<Self>;
}

/// Writes directly into one bounded output; no intermediate object tree.
pub(super) struct Encoder {
    bytes: Vec<u8>,
    limit: usize,
    depth: usize,
}

impl Encoder {
    pub(super) fn new(limit: usize) -> Self {
        Self { bytes: Vec::with_capacity(limit.min(1024)), limit, depth: 0 }
    }

    fn room(&self, count: usize) -> Result<()> {
        if self.bytes.len().checked_add(count).is_none_or(|size| size > self.limit) {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(())
    }

    pub(super) fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        self.room(bytes.len())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub(super) fn text(&mut self, value: &str) -> Result<()> {
        // Even an unescaped string must fit before it is traversed or copied.
        self.room(value.len().checked_add(2).ok_or(ProtocolError::FrameTooLarge)?)?;
        self.raw(b"\"")?;
        for character in value.chars() {
            match character {
                '"' => self.raw(br#"\""#)?,
                '\\' => self.raw(br"\\")?,
                '\u{08}' => self.raw(br"\b")?,
                '\u{0c}' => self.raw(br"\f")?,
                '\n' => self.raw(br"\n")?,
                '\r' => self.raw(br"\r")?,
                '\t' => self.raw(br"\t")?,
                control if control <= '\u{1f}' => {
                    self.raw(format!("\\u{:04x}", u32::from(control)).as_bytes())?;
                }
                other => {
                    let mut buffer = [0_u8; 4];
                    self.raw(other.encode_utf8(&mut buffer).as_bytes())?;
                }
            }
        }
        self.raw(b"\"")
    }

    pub(super) fn binary(&mut self, bytes: &[u8]) -> Result<()> {
        let length = encoded_length(bytes.len())?;
        self.room(length.checked_add(2).ok_or(ProtocolError::FrameTooLarge)?)?;
        self.text(&base64url_encode(bytes))
    }

    pub(super) fn open(&mut self, delimiter: u8) -> Result<()> {
        if self.depth >= MAX_JSON_DEPTH { return Err(ProtocolError::InvalidBody); }
        self.raw(&[delimiter])?;
        self.depth += 1;
        Ok(())
    }

    pub(super) fn close(&mut self, delimiter: u8) -> Result<()> {
        self.raw(&[delimiter])?;
        self.depth = self.depth.checked_sub(1).ok_or(ProtocolError::InvalidBody)?;
        Ok(())
    }

    pub(super) fn field(&mut self, first: &mut bool, name: &str) -> Result<()> {
        if !*first { self.raw(b",")?; }
        *first = false;
        self.text(name)?;
        self.raw(b":")
    }

    pub(super) fn tag(&mut self, name: &str) -> Result<()> {
        self.open(b'{')?;
        self.text(name)?;
        self.raw(b":")
    }

    pub(super) fn finish(self) -> Result<Vec<u8>> {
        if self.depth != 0 { return Err(ProtocolError::InvalidBody); }
        Ok(self.bytes)
    }
}

pub(super) struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
    depth: usize,
}

impl<'a> Decoder<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0, depth: 0 }
    }

    pub(super) fn literal(&mut self, expected: &[u8]) -> Result<()> {
        let end = self.position.checked_add(expected.len()).ok_or(ProtocolError::InvalidBody)?;
        if self.bytes.get(self.position..end) != Some(expected) {
            return Err(ProtocolError::InvalidBody);
        }
        self.position = end;
        Ok(())
    }

    pub(super) fn peek(&self) -> Option<u8> { self.bytes.get(self.position).copied() }

    pub(super) fn open(&mut self, delimiter: u8) -> Result<()> {
        if self.depth >= MAX_JSON_DEPTH { return Err(ProtocolError::InvalidBody); }
        self.literal(&[delimiter])?;
        self.depth += 1;
        Ok(())
    }

    pub(super) fn close(&mut self, delimiter: u8) -> Result<()> {
        self.literal(&[delimiter])?;
        self.depth = self.depth.checked_sub(1).ok_or(ProtocolError::InvalidBody)?;
        Ok(())
    }

    pub(super) fn field(&mut self, first: &mut bool, name: &str) -> Result<()> {
        if !*first { self.literal(b",")?; }
        *first = false;
        self.literal(b"\"")?;
        self.literal(name.as_bytes())?;
        self.literal(b"\":")
    }

    /// Only the escaping emitted by Encoder is accepted. Unicode is UTF-8,
    /// never an alternate escaped spelling; short control escapes are exact.
    pub(super) fn text(&mut self, maximum: usize) -> Result<String> {
        self.literal(b"\"")?;
        let mut value = Vec::new();
        loop {
            let byte = self.peek().ok_or(ProtocolError::InvalidBody)?;
            self.position += 1;
            let decoded = match byte {
                b'"' => return String::from_utf8(value).map_err(|_| ProtocolError::InvalidBody),
                b'\\' => {
                    let escaped = self.peek().ok_or(ProtocolError::InvalidBody)?;
                    self.position += 1;
                    match escaped {
                        b'"' | b'\\' => escaped,
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            self.literal(b"00")?;
                            let high = self.hex_digit()?;
                            let low = self.hex_digit()?;
                            let code = (high << 4) | low;
                            if code > 31 || matches!(code, 8 | 9 | 10 | 12 | 13) {
                                return Err(ProtocolError::InvalidBody);
                            }
                            code
                        }
                        _ => return Err(ProtocolError::InvalidBody),
                    }
                }
                0..=31 => return Err(ProtocolError::InvalidBody),
                other => other,
            };
            if value.len() >= maximum { return Err(ProtocolError::FrameTooLarge); }
            value.push(decoded);
        }
    }

    fn hex_digit(&mut self) -> Result<u8> {
        let byte = self.peek().ok_or(ProtocolError::InvalidBody)?;
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => return Err(ProtocolError::InvalidBody),
        };
        self.position += 1;
        Ok(digit)
    }

    pub(super) fn binary(&mut self, maximum: usize) -> Result<Vec<u8>> {
        let encoded = self.text(encoded_length(maximum)?)?;
        let value = base64url_decode(&encoded).map_err(|_| ProtocolError::InvalidBody)?;
        if value.len() > maximum { return Err(ProtocolError::FrameTooLarge); }
        if base64url_encode(&value) != encoded { return Err(ProtocolError::InvalidBody); }
        Ok(value)
    }

    pub(super) fn number(&mut self) -> Result<&'a str> {
        let start = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()
            || matches!(byte, b'-' | b'+' | b'e' | b'E' | b'.'))
        {
            self.position += 1;
        }
        if start == self.position { return Err(ProtocolError::InvalidBody); }
        core::str::from_utf8(&self.bytes[start..self.position]).map_err(|_| ProtocolError::InvalidBody)
    }

    pub(super) fn tag(&mut self) -> Result<String> {
        self.open(b'{')?;
        let tag = self.text(64)?;
        self.literal(b":")?;
        Ok(tag)
    }

    pub(super) fn finish(self) -> Result<()> {
        if self.position != self.bytes.len() || self.depth != 0 {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }
}

fn encoded_length(bytes: usize) -> Result<usize> {
    bytes.checked_div(3).and_then(|groups| groups.checked_mul(4))
        .and_then(|length| length.checked_add(match bytes % 3 { 0 => 0, 1 => 2, _ => 3 }))
        .ok_or(ProtocolError::FrameTooLarge)
}

/// This table describes wire fields on the existing contract type, not a new
/// parallel DTO. Exact field order also rejects missing/unknown/duplicate keys.
macro_rules! record {
    ($name:ident { $($field:ident),+ $(,)? }) => {
        record!($name { $($field),+ } => |_: &$name| Ok::<(), ProtocolError>(()));
    };
    ($name:ident { $($field:ident),+ $(,)? } => $validate:expr) => {
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> {
                ($validate)(self)?;
                output.open(b'{')?;
                let mut first = true;
                $(output.field(&mut first, stringify!($field))?; self.$field.put(output)?;)+
                output.close(b'}')
            }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                input.open(b'{')?;
                let mut first = true;
                let value = Self { $($field: {
                    input.field(&mut first, stringify!($field))?;
                    Schema::get(input)?
                }),+ };
                input.close(b'}')?;
                ($validate)(&value)?;
                Ok(value)
            }
        }
    };
}

macro_rules! tagged {
    ($name:ident { $($variant:ident => $tag:literal),+ $(,)? }) => {
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> {
                match self { $(Self::$variant(value) => {
                    output.tag($tag)?; value.put(output)?;
                }),+ }
                output.close(b'}')
            }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                let value = match input.tag()?.as_str() {
                    $($tag => Self::$variant(Schema::get(input)?),)+
                    _ => return Err(ProtocolError::InvalidBody),
                };
                input.close(b'}')?;
                Ok(value)
            }
        }
    };
}

pub(super) use {record, tagged};
