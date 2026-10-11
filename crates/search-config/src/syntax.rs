//! Private finite event-to-builder adapter; donor grammar, Eliot document shape.

use super::decode::{self, Errors, Failure};
use super::{ConfigDocument, DocumentBuilder, TextPopulation};
use crate::{
    CONFIG_TOML_SCHEMA_VERSION, ConfigError, ConfigKeyPath, ConfigLimits, ConfigSource,
    DocumentValue, LayerOperation,
};
use search_contracts::ProfileId;
use toml_parser::decoder::Encoding;
use toml_parser::lexer::Token;
use toml_parser::parser::{Event, EventKind, EventReceiver, RecursionGuard, ValidateWhitespace};
use toml_parser::{ErrorSink, Raw, Source, Span};

fn tokens(source: Source<'_>, limits: ConfigLimits) -> Result<Vec<Token>, ConfigError> {
    let limit = limits
        .max_lex_tokens
        .min(limits.max_lex_token_storage_bytes / size_of::<Token>());
    if limit == 0 {
        return Err(ConfigError::CapacityExceeded);
    }
    let mut tokens = Vec::new();
    for token in source.lex() {
        if tokens.len() >= limit {
            return Err(ConfigError::CapacityExceeded);
        }
        if tokens.len() == tokens.capacity() {
            let next = if tokens.is_empty() {
                32.min(limit)
            } else {
                tokens.len().checked_mul(2).unwrap_or(limit).min(limit)
            };
            tokens
                .try_reserve_exact(next - tokens.len())
                .map_err(|_| ConfigError::CapacityExceeded)?;
        }
        tokens.push(token);
    }
    Ok(tokens)
}

pub(super) fn parse(
    text: &str,
    captured: ConfigSource,
    limits: ConfigLimits,
) -> Result<ConfigDocument, ConfigError> {
    let source = Source::new(text);
    let tokens = tokens(source, limits)?;
    let failure = Failure::default();
    let mut receiver = Receiver {
        source,
        failure: &failure,
        builder: DocumentBuilder::new(limits),
        limits,
        schema: None,
        profile: None,
        current_section: None,
        table_open: false,
        key: None,
        target: None,
        list: None,
    };
    {
        let mut whitespace = ValidateWhitespace::new(&mut receiver, source);
        let mut guard = RecursionGuard::new(&mut whitespace, 1);
        toml_parser::parser::parse_document(&tokens, &mut guard, &mut Errors(&failure));
    }
    failure.check()?;
    receiver.finish(captured)
}

enum Target {
    Schema,
    Profile,
    Field(ConfigKeyPath),
}
struct Receiver<'i, 'f> {
    source: Source<'i>,
    failure: &'f Failure,
    builder: DocumentBuilder,
    limits: ConfigLimits,
    schema: Option<u32>,
    profile: Option<ProfileId>,
    current_section: Option<&'i str>,
    table_open: bool,
    key: Option<&'i str>,
    target: Option<Target>,
    list: Option<Vec<String>>,
}

impl<'i> Receiver<'i, '_> {
    fn receive(&mut self, action: impl FnOnce(&mut Self) -> Result<(), ConfigError>) {
        if self.failure.check().is_ok()
            && let Err(error) = action(self)
        {
            self.failure.reject(error);
        }
    }

    fn raw(
        &self,
        kind: EventKind,
        span: Span,
        encoding: Option<Encoding>,
    ) -> Result<Raw<'i>, ConfigError> {
        self.source
            .get(Event::new_unchecked(kind, encoding, span))
            .ok_or(ConfigError::UnsupportedSyntax)
    }

    fn bare_key(&self, span: Span, encoding: Option<Encoding>) -> Result<&'i str, ConfigError> {
        if encoding.is_some() {
            return Err(ConfigError::UnsupportedSyntax);
        }
        let raw = self.raw(EventKind::SimpleKey, span, encoding)?;
        raw.decode_key(&mut (), &mut Errors(self.failure));
        self.failure.check()?;
        Ok(raw.as_str())
    }

    fn open_table(&mut self) -> Result<(), ConfigError> {
        if self.table_open || self.key.is_some() || self.target.is_some() || self.list.is_some() {
            return Err(ConfigError::UnsupportedSyntax);
        }
        self.schema.ok_or(ConfigError::UnsupportedSchemaVersion)?;
        self.profile
            .as_ref()
            .ok_or(ConfigError::ProfileNotAuthorized)?;
        self.table_open = true;
        Ok(())
    }

    fn close_table(&mut self) -> Result<(), ConfigError> {
        if !self.table_open {
            return Err(ConfigError::UnsupportedSyntax);
        }
        let name = self.key.take().ok_or(ConfigError::UnsupportedSyntax)?;
        if matches!(name, "schema_version" | "profile") {
            return Err(ConfigError::DuplicateTable);
        }
        self.builder.declare_table(name)?;
        self.current_section = Some(name);
        self.table_open = false;
        Ok(())
    }

    fn select_target(&mut self) -> Result<(), ConfigError> {
        if self.table_open || self.target.is_some() || self.list.is_some() {
            return Err(ConfigError::UnsupportedSyntax);
        }
        let key = self.key.take().ok_or(ConfigError::UnsupportedSyntax)?;
        let target = if let Some(section) = self.current_section {
            Target::Field(self.builder.parsed_path(section, key)?)
        } else {
            match key {
                "schema_version" if self.schema.is_none() => Target::Schema,
                "profile" if self.profile.is_none() => Target::Profile,
                "schema_version" | "profile" => return Err(ConfigError::DuplicateKey),
                _ => return Err(ConfigError::UnknownKey),
            }
        };
        self.target = Some(target);
        Ok(())
    }

    fn complete_field(&mut self, value: DocumentValue) -> Result<(), ConfigError> {
        let Some(Target::Field(path)) = self.target.take() else {
            return Err(ConfigError::UnsupportedSyntax);
        };
        self.builder.commit_entry(path, LayerOperation::Set(value))
    }

    fn scalar_value(&mut self, span: Span, encoding: Option<Encoding>) -> Result<(), ConfigError> {
        if matches!(
            encoding,
            Some(Encoding::MlBasicString | Encoding::MlLiteralString)
        ) {
            return Err(ConfigError::UnsupportedSyntax);
        }
        let raw = self.raw(EventKind::Scalar, span, encoding)?;
        if self.list.is_some() {
            if encoding.is_none() {
                return Err(ConfigError::UnsupportedSyntax);
            }
            let len = self.list.as_ref().map_or(0, Vec::len);
            self.builder.charge_list_item(len)?;
            let text = decode::text(raw, self.failure, self.limits.max_text_bytes, |bytes| {
                self.builder.charge_text(bytes, TextPopulation::Value)
            })?;
            let list = self.list.as_mut().ok_or(ConfigError::UnsupportedSyntax)?;
            list.try_reserve_exact(1)
                .map_err(|_| ConfigError::CapacityExceeded)?;
            list.push(text);
            return Ok(());
        }
        match self.target.as_ref().ok_or(ConfigError::UnsupportedSyntax)? {
            Target::Schema => {
                let DocumentValue::Integer(value) = decode::scalar(raw, self.failure)? else {
                    return Err(ConfigError::UnsupportedSchemaVersion);
                };
                let version =
                    u32::try_from(value).map_err(|_| ConfigError::UnsupportedSchemaVersion)?;
                if version != CONFIG_TOML_SCHEMA_VERSION {
                    return Err(ConfigError::UnsupportedSchemaVersion);
                }
                self.schema = Some(version);
                self.target = None;
            }
            Target::Profile => {
                if encoding.is_none() {
                    return Err(ConfigError::ProfileNotAuthorized);
                }
                let text = decode::text(raw, self.failure, self.limits.max_text_bytes, |bytes| {
                    self.builder.charge_text(bytes, TextPopulation::Profile)
                })?;
                self.profile =
                    Some(ProfileId::new(text).map_err(|_| ConfigError::InvalidIdentifier)?);
                self.target = None;
            }
            Target::Field(_) => {
                let value = if encoding.is_some() {
                    DocumentValue::Text(decode::text(
                        raw,
                        self.failure,
                        self.limits.max_text_bytes,
                        |bytes| self.builder.charge_text(bytes, TextPopulation::Value),
                    )?)
                } else {
                    let value = decode::scalar(raw, self.failure)?;
                    let bytes = if matches!(value, DocumentValue::Boolean(_)) {
                        1
                    } else {
                        8
                    };
                    self.builder.charge_scalar(bytes)?;
                    value
                };
                self.complete_field(value)?;
            }
        }
        Ok(())
    }

    fn finish(self, source: ConfigSource) -> Result<ConfigDocument, ConfigError> {
        self.failure.check()?;
        if self.table_open || self.key.is_some() || self.target.is_some() || self.list.is_some() {
            return Err(ConfigError::UnsupportedSyntax);
        }
        let schema = self.schema.ok_or(ConfigError::UnsupportedSchemaVersion)?;
        let profile = self.profile.ok_or(ConfigError::ProfileNotAuthorized)?;
        Ok(self.builder.finish(schema, Some(profile), source))
    }
}

impl EventReceiver for Receiver<'_, '_> {
    fn std_table_open(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.receive(Self::open_table);
    }
    fn std_table_close(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.receive(Self::close_table);
    }
    fn array_table_open(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.failure.reject(ConfigError::UnsupportedSyntax);
    }
    fn inline_table_open(&mut self, _: Span, _: &mut dyn ErrorSink) -> bool {
        self.failure.reject(ConfigError::UnsupportedSyntax);
        false
    }
    fn array_open(&mut self, _: Span, _: &mut dyn ErrorSink) -> bool {
        self.receive(|receiver| {
            if receiver.list.is_some() || !matches!(receiver.target, Some(Target::Field(_))) {
                return Err(ConfigError::UnsupportedSyntax);
            }
            receiver.list = Some(Vec::new());
            Ok(())
        });
        self.failure.check().is_ok()
    }
    fn array_close(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.receive(|receiver| {
            let list = receiver.list.take().ok_or(ConfigError::UnsupportedSyntax)?;
            receiver.complete_field(DocumentValue::StringList(list))
        });
    }
    fn simple_key(&mut self, span: Span, encoding: Option<Encoding>, _: &mut dyn ErrorSink) {
        self.receive(|receiver| {
            if receiver.key.is_some() || receiver.target.is_some() || receiver.list.is_some() {
                return Err(ConfigError::UnsupportedSyntax);
            }
            receiver.key = Some(receiver.bare_key(span, encoding)?);
            Ok(())
        });
    }
    fn key_sep(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.failure.reject(ConfigError::UnsupportedSyntax);
    }
    fn key_val_sep(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.receive(Self::select_target);
    }
    fn scalar(&mut self, span: Span, encoding: Option<Encoding>, _: &mut dyn ErrorSink) {
        self.receive(|receiver| receiver.scalar_value(span, encoding));
    }
    fn error(&mut self, _: Span, _: &mut dyn ErrorSink) {
        self.failure.reject(ConfigError::UnsupportedSyntax);
    }
}
