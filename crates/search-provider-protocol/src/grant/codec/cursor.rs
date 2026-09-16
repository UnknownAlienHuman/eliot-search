//! Bounded byte cursor for the canonical grant-request body.

use search_contracts::{
    CorpusId, CorpusOrPortfolioId, ReferencePortfolioId, MAX_SET_ITEMS,
};

use crate::error::ProtocolError;

pub(super) struct Cursor<'a> {
    body: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(body: &'a [u8]) -> Self {
        Self { body, position: 0 }
    }

    pub(super) fn expect(&mut self, literal: &[u8]) -> Result<(), ProtocolError> {
        let end = self.position.saturating_add(literal.len());
        if end <= self.body.len() && &self.body[self.position..end] == literal {
            self.position = end;
            Ok(())
        } else {
            Err(ProtocolError::InvalidBody)
        }
    }

    pub(super) fn parse_u64(&mut self) -> Result<u64, ProtocolError> {
        let start = self.position;
        while self
            .body
            .get(self.position)
            .is_some_and(u8::is_ascii_digit)
        {
            self.position += 1;
        }
        let digits = &self.body[start..self.position];
        if digits.is_empty() || digits.len() > 20 || (digits.len() > 1 && digits[0] == b'0') {
            return Err(ProtocolError::InvalidBody);
        }
        let mut value = 0_u64;
        for digit in digits {
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(u64::from(*digit - b'0')))
                .ok_or(ProtocolError::InvalidBody)?;
        }
        Ok(value)
    }

    pub(super) fn parse_bool(&mut self) -> Result<bool, ProtocolError> {
        if self.body[self.position..].starts_with(b"true") {
            self.position += 4;
            Ok(true)
        } else if self.body[self.position..].starts_with(b"false") {
            self.position += 5;
            Ok(false)
        } else {
            Err(ProtocolError::InvalidBody)
        }
    }

    pub(super) fn parse_array<T>(
        &mut self,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, ProtocolError>,
    ) -> Result<Vec<T>, ProtocolError> {
        self.expect(b"[")?;
        let mut items = Vec::new();
        if self.body.get(self.position) == Some(&b']') {
            self.position += 1;
            return Ok(items);
        }
        loop {
            if items.len() >= MAX_SET_ITEMS {
                return Err(ProtocolError::ResourceExhausted);
            }
            items.push(parse_item(self)?);
            match self.body.get(self.position) {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(items);
                }
                _ => return Err(ProtocolError::InvalidBody),
            }
        }
    }

    pub(super) fn parse_uuid<T>(
        &mut self,
        constructor: impl FnOnce([u8; 16]) -> T,
    ) -> Result<T, ProtocolError> {
        let token = self.parse_quoted_token(32)?;
        let bytes = decode_hex_exact::<16>(token)?;
        Ok(constructor(bytes))
    }

    pub(super) fn parse_target(&mut self) -> Result<CorpusOrPortfolioId, ProtocolError> {
        let token = self.parse_quoted_token(34)?;
        if token.len() != 34 || token[1] != b':' {
            return Err(ProtocolError::InvalidBody);
        }
        let bytes = decode_hex_exact::<16>(&token[2..])?;
        match token[0] {
            b'c' => Ok(CorpusOrPortfolioId::Corpus(CorpusId::from_bytes(bytes))),
            b'p' => Ok(CorpusOrPortfolioId::Portfolio(
                ReferencePortfolioId::from_bytes(bytes),
            )),
            _ => Err(ProtocolError::InvalidBody),
        }
    }

    pub(super) fn parse_quoted_token(&mut self, maximum: usize) -> Result<&'a [u8], ProtocolError> {
        self.expect(b"\"")?;
        let token = self.parse_token_until_quote(maximum)?;
        self.expect(b"\"")?;
        Ok(token)
    }

    pub(super) fn parse_token_until_quote(&mut self, maximum: usize) -> Result<&'a [u8], ProtocolError> {
        let start = self.position;
        while let Some(byte) = self.body.get(self.position) {
            if *byte == b'"' {
                break;
            }
            if *byte < 0x20 || *byte == b'\\' {
                return Err(ProtocolError::InvalidBody);
            }
            self.position += 1;
            if self.position - start > maximum {
                return Err(ProtocolError::InvalidBody);
            }
        }
        let token = &self.body[start..self.position];
        if token.is_empty() || self.body.get(self.position) != Some(&b'"') {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(token)
    }

    pub(super) fn parse_hex_bytes(&mut self, maximum_bytes: usize) -> Result<Vec<u8>, ProtocolError> {
        let start = self.position;
        while self
            .body
            .get(self.position)
            .is_some_and(|byte| lower_hex(*byte).is_some())
        {
            self.position += 1;
        }
        let token = &self.body[start..self.position];
        if token.is_empty() || !token.len().is_multiple_of(2) || token.len() / 2 > maximum_bytes {
            return Err(ProtocolError::InvalidBody);
        }
        let mut output = Vec::with_capacity(token.len() / 2);
        for pair in token.chunks_exact(2) {
            output.push((lower_hex(pair[0]).ok_or(ProtocolError::InvalidBody)? << 4)
                | lower_hex(pair[1]).ok_or(ProtocolError::InvalidBody)?);
        }
        Ok(output)
    }

    pub(super) const fn is_exhausted(&self) -> bool {
        self.position == self.body.len()
    }
}

fn decode_hex_exact<const N: usize>(token: &[u8]) -> Result<[u8; N], ProtocolError> {
    if token.len() != N * 2 {
        return Err(ProtocolError::InvalidBody);
    }
    let mut output = [0_u8; N];
    for (index, pair) in token.chunks_exact(2).enumerate() {
        output[index] = (lower_hex(pair[0]).ok_or(ProtocolError::InvalidBody)? << 4)
            | lower_hex(pair[1]).ok_or(ProtocolError::InvalidBody)?;
    }
    Ok(output)
}

const fn lower_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
