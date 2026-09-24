//! Syntax validation over one already length-bounded UTF-8 JSON value.
//!
//! This scanner does not deserialize, normalize or copy the body. An explicit
//! container stack avoids recursion on untrusted nesting; its size is bounded
//! by the input length already checked by the frame owner. Allocation failures
//! are typed refusals. Envelope tags, duplicate fields, canonical spelling and
//! field-level bounds remain the responsibility of the typed envelope codec.

use crate::error::ProtocolError;

#[derive(Clone, Copy)]
enum Container {
    ArrayStart,
    ArrayValue,
    ArrayNext,
    ObjectStart,
    ObjectKey,
    ObjectNext,
}

pub(super) fn validate(bytes: &[u8]) -> Result<(), ProtocolError> {
    core::str::from_utf8(bytes).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let mut input = Cursor {
        bytes,
        position: 0,
    };
    let mut containers = Vec::new();
    input.whitespace();
    input.value(&mut containers)?;
    while let Some(container) = containers.pop() {
        input.whitespace();
        match container {
            Container::ArrayStart if input.consume(b']') => {}
            Container::ArrayStart | Container::ArrayValue => {
                push(&mut containers, Container::ArrayNext)?;
                input.value(&mut containers)?;
            }
            Container::ArrayNext => {
                if !input.consume(b']') {
                    input.literal(b",")?;
                    push(&mut containers, Container::ArrayValue)?;
                }
            }
            Container::ObjectStart if input.consume(b'}') => {}
            Container::ObjectStart | Container::ObjectKey => {
                input.string()?;
                input.whitespace();
                input.literal(b":")?;
                input.whitespace();
                push(&mut containers, Container::ObjectNext)?;
                input.value(&mut containers)?;
            }
            Container::ObjectNext => {
                if !input.consume(b'}') {
                    input.literal(b",")?;
                    push(&mut containers, Container::ObjectKey)?;
                }
            }
        }
    }
    input.whitespace();
    if input.position == bytes.len() {
        Ok(())
    } else {
        Err(ProtocolError::InvalidEnvelope)
    }
}

fn push(containers: &mut Vec<Container>, next: Container) -> Result<(), ProtocolError> {
    // Every live container corresponds to an opening delimiter already consumed
    // from the finite input. This introduces no independent or renewable quota.
    containers
        .try_reserve(1)
        .map_err(|_| ProtocolError::ResourceExhausted)?;
    containers.push(next);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn take(&mut self) -> Result<u8, ProtocolError> {
        let byte = self.peek().ok_or(ProtocolError::InvalidEnvelope)?;
        self.position += 1;
        Ok(byte)
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), ProtocolError> {
        if self.bytes[self.position..].starts_with(literal) {
            self.position += literal.len();
            Ok(())
        } else {
            Err(ProtocolError::InvalidEnvelope)
        }
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn value(&mut self, containers: &mut Vec<Container>) -> Result<(), ProtocolError> {
        match self.peek() {
            Some(b'{') => {
                self.position += 1;
                push(containers, Container::ObjectStart)
            }
            Some(b'[') => {
                self.position += 1;
                push(containers, Container::ArrayStart)
            }
            Some(b'"') => self.string(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(ProtocolError::InvalidEnvelope),
        }
    }

    fn string(&mut self) -> Result<(), ProtocolError> {
        self.literal(b"\"")?;
        loop {
            match self.take()? {
                b'"' => return Ok(()),
                b'\\' => match self.take()? {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                    b'u' => {
                        // JSON syntax permits any four hexadecimal digits.
                        // Unicode scalar/canonical spelling rules belong to the
                        // field decoder; this scanner never rewrites escapes.
                        for _ in 0..4 {
                            if !self.take()?.is_ascii_hexdigit() {
                                return Err(ProtocolError::InvalidEnvelope);
                            }
                        }
                    }
                    _ => return Err(ProtocolError::InvalidEnvelope),
                },
                0x00..=0x1f => return Err(ProtocolError::InvalidEnvelope),
                _ => {}
            }
        }
    }

    fn number(&mut self) -> Result<(), ProtocolError> {
        self.consume(b'-');
        match self.take()? {
            b'0' => {}
            b'1'..=b'9' => self.digits(),
            _ => return Err(ProtocolError::InvalidEnvelope),
        }
        if self.consume(b'.') {
            self.required_digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            self.required_digits()?;
        }
        // A leading-zero suffix or a non-delimiter is rejected by the enclosing
        // state (including root completion), without parsing into a machine
        // numeric type or silently rounding a value.
        Ok(())
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
    }

    fn required_digits(&mut self) -> Result<(), ProtocolError> {
        if !self.take()?.is_ascii_digit() {
            return Err(ProtocolError::InvalidEnvelope);
        }
        self.digits();
        Ok(())
    }
}
