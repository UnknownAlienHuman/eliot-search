//! Private maintained decoder sinks. No local string or number grammar.

use crate::{ConfigError, DocumentValue};
use std::cell::Cell;
use toml_parser::decoder::{IntegerRadix, ScalarKind, StringBuilder};
use toml_parser::{ErrorSink, ParseError, Raw};

#[derive(Default)]
pub(super) struct Failure(Cell<Option<ConfigError>>);

impl Failure {
    pub(super) fn reject(&self, reason: ConfigError) {
        if self.0.get().is_none() {
            self.0.set(Some(reason));
        }
    }
    pub(super) fn check(&self) -> Result<(), ConfigError> {
        self.0.get().map_or(Ok(()), Err)
    }
}

pub(super) struct Errors<'a>(pub(super) &'a Failure);
impl ErrorSink for Errors<'_> {
    fn report_error(&mut self, _: ParseError) {
        self.0.reject(ConfigError::UnsupportedSyntax);
    }
}

struct Counter<'a> {
    bytes: usize,
    limit: usize,
    failure: &'a Failure,
}
impl Counter<'_> {
    fn append(&mut self, bytes: usize) -> bool {
        if self.failure.check().is_err() {
            return false;
        }
        let Some(next) = self
            .bytes
            .checked_add(bytes)
            .filter(|next| *next <= self.limit)
        else {
            self.failure.reject(ConfigError::CapacityExceeded);
            return false;
        };
        self.bytes = next;
        true
    }
}
impl<'s> StringBuilder<'s> for Counter<'_> {
    fn clear(&mut self) {
        if self.failure.check().is_ok() {
            self.bytes = 0;
        }
    }
    fn push_str(&mut self, append: &'s str) -> bool {
        self.append(append.len())
    }
    fn push_char(&mut self, append: char) -> bool {
        self.append(append.len_utf8())
    }
}

struct Writer<'a> {
    text: String,
    exact_bytes: usize,
    failure: &'a Failure,
}
impl Writer<'_> {
    fn admits(&self, bytes: usize) -> bool {
        if self.failure.check().is_err() {
            return false;
        }
        if self
            .text
            .len()
            .checked_add(bytes)
            .is_none_or(|next| next > self.exact_bytes)
        {
            self.failure.reject(ConfigError::CapacityExceeded);
            return false;
        }
        true
    }
}
impl<'s> StringBuilder<'s> for Writer<'_> {
    fn clear(&mut self) {
        if self.failure.check().is_ok() {
            self.text.clear();
        }
    }
    fn push_str(&mut self, append: &'s str) -> bool {
        if !self.admits(append.len()) {
            return false;
        }
        self.text.push_str(append);
        true
    }
    fn push_char(&mut self, append: char) -> bool {
        if !self.admits(append.len_utf8()) {
            return false;
        }
        self.text.push(append);
        true
    }
}

pub(super) fn text(
    raw: Raw<'_>,
    failure: &Failure,
    ceiling: usize,
    charge: impl FnOnce(usize) -> Result<(), ConfigError>,
) -> Result<String, ConfigError> {
    failure.check()?;
    let mut errors = Errors(failure);
    let mut counter = Counter {
        bytes: 0,
        limit: ceiling,
        failure,
    };
    let kind = raw.decode_scalar(&mut counter, &mut errors);
    failure.check()?;
    if kind != ScalarKind::String {
        return Err(ConfigError::UnsupportedSyntax);
    }
    let exact_bytes = counter.bytes;
    charge(exact_bytes)?;
    let mut output = String::new();
    output
        .try_reserve_exact(exact_bytes)
        .map_err(|_| ConfigError::CapacityExceeded)?;
    let mut writer = Writer {
        text: output,
        exact_bytes,
        failure,
    };
    let kind = raw.decode_scalar(&mut writer, &mut errors);
    failure.check()?;
    if kind != ScalarKind::String || writer.text.len() != exact_bytes {
        return Err(ConfigError::UnsupportedSyntax);
    }
    Ok(writer.text)
}

// Fixed stack space for normalized decimal i64; never a donor heap tree.
struct Number<'a> {
    bytes: [u8; 32],
    len: usize,
    failure: &'a Failure,
}
impl Number<'_> {
    fn append(&mut self, text: &str) -> bool {
        if self.failure.check().is_err() {
            return false;
        }
        let Some(next) = self
            .len
            .checked_add(text.len())
            .filter(|next| *next <= self.bytes.len())
        else {
            self.failure.reject(ConfigError::ValueOutOfBounds);
            return false;
        };
        self.bytes[self.len..next].copy_from_slice(text.as_bytes());
        self.len = next;
        true
    }
}
impl<'s> StringBuilder<'s> for Number<'_> {
    fn clear(&mut self) {
        if self.failure.check().is_ok() {
            self.len = 0;
        }
    }
    fn push_str(&mut self, text: &'s str) -> bool {
        self.append(text)
    }
    fn push_char(&mut self, character: char) -> bool {
        self.append(character.encode_utf8(&mut [0; 4]))
    }
}

pub(super) fn scalar(raw: Raw<'_>, failure: &Failure) -> Result<DocumentValue, ConfigError> {
    failure.check()?;
    let mut errors = Errors(failure);
    let kind = raw.decode_scalar(&mut (), &mut errors);
    failure.check()?;
    match kind {
        ScalarKind::Boolean(value) => Ok(DocumentValue::Boolean(value)),
        ScalarKind::Integer(IntegerRadix::Dec) => {
            let mut number = Number {
                bytes: [0; 32],
                len: 0,
                failure,
            };
            if raw.decode_scalar(&mut number, &mut errors) != kind {
                return Err(ConfigError::UnsupportedSyntax);
            }
            failure.check()?;
            let normalized = std::str::from_utf8(&number.bytes[..number.len])
                .map_err(|_| ConfigError::UnsupportedSyntax)?;
            normalized
                .parse::<i64>()
                .map(DocumentValue::Integer)
                .map_err(|_| ConfigError::ValueOutOfBounds)
        }
        _ => Err(ConfigError::UnsupportedSyntax),
    }
}

#[cfg(test)]
mod tests {
    // AUTHORED_NOT_RUN; #352 defers execution to integrated #238/#215 qualification.
    use super::*;
    use toml_parser::Source;

    // Tiny fixture scalars use the actual lexer token encoding and span.
    fn raw(input: &str) -> Raw<'_> {
        let source = Source::new(input);
        let token = source.lex().next().expect("scalar token");
        let raw = source.get(token).expect("lexer span");
        assert_eq!(raw.as_str(), input);
        raw
    }

    #[test]
    fn counter_failure_preserves_first_reason_and_survives_clear() {
        let failure = Failure::default();
        let mut counter = Counter {
            bytes: 0,
            limit: 2,
            failure: &failure,
        };
        assert!(counter.push_str("ab"));
        assert!(!counter.push_char('é'));
        failure.reject(ConfigError::UnsupportedSyntax);
        counter.clear();
        assert!(!counter.push_str("x"));
        assert_eq!(counter.bytes, 2);
        assert_eq!(failure.check(), Err(ConfigError::CapacityExceeded));
    }

    #[test]
    fn writer_failure_cannot_clear_or_append_partial_output() {
        let failure = Failure::default();
        let mut writer = Writer {
            text: String::with_capacity(2),
            exact_bytes: 2,
            failure: &failure,
        };
        assert!(writer.push_str("ab"));
        assert!(!writer.push_char('é'));
        writer.clear();
        assert!(!writer.push_str("x"));
        assert_eq!(writer.text, "ab");
        assert_eq!(failure.check(), Err(ConfigError::CapacityExceeded));
    }

    #[test]
    fn number_failure_keeps_its_fixed_stack_extent_after_clear() {
        let failure = Failure::default();
        let mut number = Number {
            bytes: [0; 32],
            len: 0,
            failure: &failure,
        };
        assert!(number.push_str("1"));
        assert!(!number.push_str("999999999999999999999999999999999"));
        number.clear();
        assert!(!number.push_char('2'));
        assert_eq!(number.len, 1);
        assert_eq!(number.bytes[0], b'1');
        assert_eq!(failure.check(), Err(ConfigError::ValueOutOfBounds));
    }

    #[test]
    fn donor_unicode_decode_charges_exact_utf8_before_fill() {
        let failure = Failure::default();
        let mut charged = None;
        let decoded = text(raw(r#""\u00e9""#), &failure, 2, |bytes| {
            charged = Some(bytes);
            Ok(())
        })
        .expect("decoded Unicode");
        assert_eq!(charged, Some(2));
        assert_eq!(decoded, "é");
        assert_eq!(failure.check(), Ok(()));
    }

    #[test]
    fn refused_donor_count_never_reaches_charge_or_later_success() {
        let failure = Failure::default();
        let mut charged = false;
        assert_eq!(
            text(raw(r#""\u00e9x""#), &failure, 1, |_| {
                charged = true;
                Ok(())
            }),
            Err(ConfigError::CapacityExceeded)
        );
        assert!(!charged);
        assert_eq!(
            text(raw(r#""ok""#), &failure, 64, |_| {
                charged = true;
                Ok(())
            }),
            Err(ConfigError::CapacityExceeded)
        );
        assert!(!charged);
    }

    #[test]
    fn malformed_donor_escape_is_content_free_and_terminal() {
        for input in [r#""private-\q-marker""#, r#""\uD800""#] {
            let failure = Failure::default();
            let mut charged = false;
            let error = text(raw(input), &failure, 64, |_| {
                charged = true;
                Ok(())
            })
            .expect_err("invalid donor escape");
            assert_eq!(error, ConfigError::UnsupportedSyntax);
            assert!(!charged);
            assert!(!error.to_string().contains("private"));
            assert!(!format!("{error:?}").contains("D800"));
            assert_eq!(
                scalar(raw("true"), &failure),
                Err(ConfigError::UnsupportedSyntax)
            );
        }
    }
}
