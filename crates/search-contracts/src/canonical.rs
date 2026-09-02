use crate::bounds::{
    BoundedBytes, BoundedCanonicalBytes, BoundedList, BoundedMap, BoundedOpaqueBytes, BoundedText,
    MAX_BEHAVIOR_SIGNATURE_BYTES, MAX_CANONICAL_BYTES, MAX_CANONICAL_DEPTH,
    MAX_CANONICAL_KEY_BYTES, MAX_COLLECTION_ITEMS, MAX_DISPLAY_NAME_BYTES, MAX_DISPLAY_PATH_BYTES,
    MAX_EXPRESSION_BYTES, MAX_FACET_VALUE_BYTES, MAX_MAP_ENTRIES, MAX_METADATA_ENTRIES,
    MAX_METADATA_KEY_BYTES, MAX_NAME_BYTES, MAX_OBSERVATION_BYTES, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_RAW_BYTES, MAX_SYMBOL_KEY_BYTES,
};
use crate::{Blake3Digest32, ContractError, ContractErrorKind, FusionProfileId, ProfileId};
use core::fmt;

pub type CanonicalText = BoundedText<MAX_RAW_BYTES>;
pub type CanonicalKey = BoundedText<MAX_CANONICAL_KEY_BYTES>;
pub type CanonicalBytes = BoundedCanonicalBytes<MAX_CANONICAL_BYTES>;
pub type OpaqueBytes = BoundedOpaqueBytes<MAX_OPAQUE_REF_BYTES>;

macro_rules! text_wrapper {
    ($name:ident, $limit:expr) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(BoundedText<$limit>);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                BoundedText::new_non_empty(value).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

text_wrapper!(OpaqueId, MAX_OPAQUE_ID_BYTES);
text_wrapper!(OpaqueRef, MAX_OPAQUE_REF_BYTES);
text_wrapper!(BoundedDisplayName, MAX_DISPLAY_NAME_BYTES);
text_wrapper!(BoundedDisplayPath, MAX_DISPLAY_PATH_BYTES);
text_wrapper!(BoundedName, MAX_NAME_BYTES);
text_wrapper!(BoundedSymbolKey, MAX_SYMBOL_KEY_BYTES);
text_wrapper!(BoundedExpression, MAX_EXPRESSION_BYTES);
text_wrapper!(BoundedObservation, MAX_OBSERVATION_BYTES);
text_wrapper!(BoundedBehaviorSignature, MAX_BEHAVIOR_SIGNATURE_BYTES);
text_wrapper!(OpaqueAuthorizedFacetValue, MAX_FACET_VALUE_BYTES);

/// Producer-validated canonical bytes. Consumers may compare or forward them,
/// but may not reinterpret them as an arbitrary package/vendor object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueCanonicalBytes(CanonicalBytes);

impl OpaqueCanonicalBytes {
    pub fn from_validated(value: impl Into<Vec<u8>>) -> Result<Self, ContractError> {
        let value = CanonicalBytes::from_validated(value)?;
        if value.is_empty() {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "opaque_canonical_bytes",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Canonical metadata key proposed by P00 clarification #48.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MetadataKey(BoundedText<MAX_METADATA_KEY_BYTES>);

impl MetadataKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = BoundedText::new_non_empty(value)?;
        let mut bytes = value.as_bytes().iter().copied();
        let first = bytes
            .next()
            .ok_or_else(|| ContractError::new(ContractErrorKind::Empty, "metadata_key"))?;
        if !first.is_ascii_lowercase()
            || bytes.any(|byte| {
                !(byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'.' | b'-'))
            })
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidCharacter,
                "metadata_key",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Closed scalar set permitted in default non-content metadata.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MetadataScalar {
    Boolean(bool),
    Unsigned(u64),
    Signed(i64),
    DurationMs(u64),
    Digest(Blake3Digest32),
    ProfileId(ProfileId),
    TemplateId(OpaqueId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedNonContentMetadata {
    pub entries: BoundedMap<MetadataKey, MetadataScalar, MAX_METADATA_ENTRIES>,
}

impl BoundedNonContentMetadata {
    pub fn new(
        entries: impl IntoIterator<Item = (MetadataKey, MetadataScalar)>,
    ) -> Result<Self, ContractError> {
        Ok(Self {
            entries: BoundedMap::from_entries(entries)?,
        })
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: BoundedMap::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactOrEntityBoost {
    None,
    ExactName,
    QualifiedName,
    EntityKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineageDiversityAction {
    Retained,
    Collapsed,
    Capped,
}

crate::impl_wire_enum!(ExactOrEntityBoost {
    None => "none",
    ExactName => "exact_name",
    QualifiedName => "qualified_name",
    EntityKind => "entity_kind",
});
crate::impl_wire_enum!(LineageDiversityAction {
    Retained => "retained",
    Collapsed => "collapsed",
    Capped => "capped",
});

/// Content-free deterministic ranking explanation. Raw vendor scores and
/// inaccessible population details are structurally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedNonContentRankingTrace {
    pub fusion_profile_id: FusionProfileId,
    pub fused_rank: u32,
    pub exact_or_entity_boost: ExactOrEntityBoost,
    pub evidence_role_priority: u16,
    pub portfolio_priority: u16,
    pub lineage_diversity_action: LineageDiversityAction,
    pub deterministic_tie_break_digest: Blake3Digest32,
}

/// Strict reader for a schema-owned canonical object. A decoder removes every
/// declared field and calls `finish`; any residual field fails closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedCanonicalObject {
    fields: BoundedMap<CanonicalKey, CanonicalValue, MAX_MAP_ENTRIES>,
    schema_field: &'static str,
}

impl ClosedCanonicalObject {
    pub fn from_value(
        value: CanonicalValue,
        schema_field: &'static str,
    ) -> Result<Self, ContractError> {
        let CanonicalValue::Object(fields) = value else {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                schema_field,
            ));
        };
        Ok(Self {
            fields,
            schema_field,
        })
    }

    pub fn take_required(&mut self, field: &'static str) -> Result<CanonicalValue, ContractError> {
        self.take_optional(field)?
            .ok_or_else(|| ContractError::new(ContractErrorKind::MalformedPayload, field))
    }

    pub fn take_optional(
        &mut self,
        field: &'static str,
    ) -> Result<Option<CanonicalValue>, ContractError> {
        let key = CanonicalKey::new_non_empty(field)?;
        Ok(self.fields.remove(&key))
    }

    pub fn finish(self) -> Result<(), ContractError> {
        if self.fields.is_empty() {
            Ok(())
        } else {
            Err(ContractError::unknown_field(self.schema_field))
        }
    }
}

/// Closed canonical data model used for deterministic JSON/CBOR identity
/// inputs. Non-negative integers use `U64`; `I64` is canonical only for
/// negative values so JSON decoding is unambiguous.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    Text(CanonicalText),
    Bytes(BoundedBytes<MAX_RAW_BYTES>),
    Array(BoundedList<Self, MAX_COLLECTION_ITEMS>),
    Object(BoundedMap<CanonicalKey, Self, MAX_MAP_ENTRIES>),
}

impl CanonicalValue {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_value(self, 0)
    }
}

/// Encode UTF-8 JSON with no insignificant whitespace and bytewise-sorted keys.
pub fn to_canonical_json(value: &CanonicalValue) -> Result<CanonicalBytes, ContractError> {
    value.validate()?;
    let mut output = Vec::new();
    encode_json(value, &mut output);
    CanonicalBytes::from_validated(output)
}

/// Parse only the exact canonical JSON representation emitted by this module.
pub fn parse_canonical_json(bytes: &[u8]) -> Result<CanonicalValue, ContractError> {
    if bytes.len() > MAX_CANONICAL_BYTES {
        return Err(ContractError::oversize(
            "canonical_json",
            MAX_CANONICAL_BYTES,
            bytes.len(),
        ));
    }
    let mut parser = JsonParser::new(bytes);
    let value = parser.parse_value(0)?;
    if parser.position != bytes.len() {
        return Err(ContractError::malformed("canonical_json"));
    }
    let encoded = to_canonical_json(&value)?;
    if encoded.as_slice() != bytes {
        return Err(ContractError::new(
            ContractErrorKind::NonCanonical,
            "canonical_json",
        ));
    }
    Ok(value)
}

/// Encode RFC 8949 deterministic CBOR: definite lengths, shortest integers,
/// and map keys ordered by encoded-key length then lexicographically.
pub fn to_canonical_cbor(value: &CanonicalValue) -> Result<CanonicalBytes, ContractError> {
    value.validate()?;
    let mut output = Vec::new();
    encode_cbor(value, &mut output)?;
    CanonicalBytes::from_validated(output)
}

/// Parse the closed deterministic CBOR subset and reject non-canonical bytes.
pub fn parse_canonical_cbor(bytes: &[u8]) -> Result<CanonicalValue, ContractError> {
    if bytes.len() > MAX_CANONICAL_BYTES {
        return Err(ContractError::oversize(
            "canonical_cbor",
            MAX_CANONICAL_BYTES,
            bytes.len(),
        ));
    }
    let mut parser = CborParser::new(bytes);
    let value = parser.parse_value(0)?;
    if parser.position != bytes.len() {
        return Err(ContractError::malformed("canonical_cbor"));
    }
    let encoded = to_canonical_cbor(&value)?;
    if encoded.as_slice() != bytes {
        return Err(ContractError::new(
            ContractErrorKind::NonCanonical,
            "canonical_cbor",
        ));
    }
    Ok(value)
}

/// Construct a bounded domain-separated hash preimage. Cryptographic hashing
/// remains an explicit caller operation; the exact bytes are deterministic.
pub fn domain_separated_preimage(
    domain: &'static str,
    value: &CanonicalValue,
) -> Result<CanonicalBytes, ContractError> {
    if domain.is_empty() || !domain.is_ascii() {
        return Err(ContractError::new(
            ContractErrorKind::InvalidCharacter,
            "domain_separator",
        ));
    }
    let payload = to_canonical_cbor(value)?;
    let total = domain.len().saturating_add(1).saturating_add(payload.len());
    if total > MAX_CANONICAL_BYTES {
        return Err(ContractError::oversize(
            "domain_separated_preimage",
            MAX_CANONICAL_BYTES,
            total,
        ));
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(domain.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(payload.as_slice());
    CanonicalBytes::from_validated(bytes)
}

fn validate_value(value: &CanonicalValue, depth: usize) -> Result<(), ContractError> {
    if depth > MAX_CANONICAL_DEPTH {
        return Err(ContractError::bounded(
            ContractErrorKind::DepthExceeded,
            "canonical_value",
            usize_to_u64(MAX_CANONICAL_DEPTH),
            usize_to_u64(depth),
        ));
    }
    match value {
        CanonicalValue::I64(integer) if *integer >= 0 => Err(ContractError::new(
            ContractErrorKind::NonCanonical,
            "canonical_integer",
        )),
        CanonicalValue::Array(values) => {
            for item in values {
                validate_value(item, depth.saturating_add(1))?;
            }
            Ok(())
        }
        CanonicalValue::Object(values) => {
            if values.len() == 1
                && values
                    .iter()
                    .next()
                    .is_some_and(|(key, _)| key.as_str() == "$bytes")
            {
                return Err(ContractError::new(
                    ContractErrorKind::NonCanonical,
                    "canonical_reserved_bytes_tag",
                ));
            }
            for (_, item) in values {
                validate_value(item, depth.saturating_add(1))?;
            }
            Ok(())
        }
        CanonicalValue::Null
        | CanonicalValue::Bool(_)
        | CanonicalValue::I64(_)
        | CanonicalValue::U64(_)
        | CanonicalValue::Text(_)
        | CanonicalValue::Bytes(_) => Ok(()),
    }
}

fn encode_json(value: &CanonicalValue, output: &mut Vec<u8>) {
    match value {
        CanonicalValue::Null => output.extend_from_slice(b"null"),
        CanonicalValue::Bool(true) => output.extend_from_slice(b"true"),
        CanonicalValue::Bool(false) => output.extend_from_slice(b"false"),
        CanonicalValue::I64(value) => output.extend_from_slice(value.to_string().as_bytes()),
        CanonicalValue::U64(value) => output.extend_from_slice(value.to_string().as_bytes()),
        CanonicalValue::Text(value) => encode_json_string(value.as_str(), output),
        CanonicalValue::Bytes(value) => {
            output.extend_from_slice(b"{\"$bytes\":");
            encode_json_string(&base64url_encode(value.as_slice()), output);
            output.push(b'}');
        }
        CanonicalValue::Array(values) => {
            output.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                encode_json(item, output);
            }
            output.push(b']');
        }
        CanonicalValue::Object(values) => {
            output.push(b'{');
            for (index, (key, item)) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                encode_json_string(key.as_str(), output);
                output.push(b':');
                encode_json(item, output);
            }
            output.push(b'}');
        }
    }
}

fn encode_json_string(value: &str, output: &mut Vec<u8>) {
    output.push(b'"');
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(br#"\""#),
            '\\' => output.extend_from_slice(br"\\"),
            '\u{08}' => output.extend_from_slice(br"\b"),
            '\u{0c}' => output.extend_from_slice(br"\f"),
            '\n' => output.extend_from_slice(br"\n"),
            '\r' => output.extend_from_slice(br"\r"),
            '\t' => output.extend_from_slice(br"\t"),
            control if control <= '\u{1f}' => {
                let code = u32::from(control);
                output.extend_from_slice(format!("\\u{code:04x}").as_bytes());
            }
            other => {
                let mut buffer = [0_u8; 4];
                output.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

fn encode_cbor(value: &CanonicalValue, output: &mut Vec<u8>) -> Result<(), ContractError> {
    match value {
        CanonicalValue::Null => output.push(0xf6),
        CanonicalValue::Bool(false) => output.push(0xf4),
        CanonicalValue::Bool(true) => output.push(0xf5),
        CanonicalValue::U64(value) => encode_cbor_head(0, *value, output),
        CanonicalValue::I64(value) => {
            let encoded = u64::try_from(-1_i128 - i128::from(*value)).map_err(|_| {
                ContractError::new(ContractErrorKind::NonCanonical, "canonical_cbor_i64")
            })?;
            encode_cbor_head(1, encoded, output);
        }
        CanonicalValue::Text(value) => {
            encode_cbor_head(3, usize_to_u64(value.len()), output);
            output.extend_from_slice(value.as_bytes());
        }
        CanonicalValue::Bytes(value) => {
            encode_cbor_head(2, usize_to_u64(value.len()), output);
            output.extend_from_slice(value.as_slice());
        }
        CanonicalValue::Array(values) => {
            encode_cbor_head(4, usize_to_u64(values.len()), output);
            for item in values {
                encode_cbor(item, output)?;
            }
        }
        CanonicalValue::Object(values) => {
            encode_cbor_head(5, usize_to_u64(values.len()), output);
            let mut entries = Vec::with_capacity(values.len());
            for (key, item) in values {
                let mut encoded_key = Vec::new();
                encode_cbor_head(3, usize_to_u64(key.len()), &mut encoded_key);
                encoded_key.extend_from_slice(key.as_bytes());
                entries.push((encoded_key, item));
            }
            entries.sort_by(|left, right| {
                left.0
                    .len()
                    .cmp(&right.0.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            for (key, item) in entries {
                output.extend_from_slice(&key);
                encode_cbor(item, output)?;
            }
        }
    }
    Ok(())
}

fn encode_cbor_head(major: u8, value: u64, output: &mut Vec<u8>) {
    let prefix = major << 5;
    match value {
        0..=23 => output.push(prefix | u8::try_from(value).expect("range checked")),
        24..=0xff => {
            output.extend_from_slice(&[prefix | 0x18, u8::try_from(value).expect("range checked")]);
        }
        0x100..=0xffff => {
            output.push(prefix | 0x19);
            output.extend_from_slice(&u16::try_from(value).expect("range checked").to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push(prefix | 0x1a);
            output.extend_from_slice(&u32::try_from(value).expect("range checked").to_be_bytes());
        }
        _ => {
            output.push(prefix | 0x1b);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> JsonParser<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn parse_value(&mut self, depth: usize) -> Result<CanonicalValue, ContractError> {
        Self::check_depth(depth)?;
        match self.peek() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(CanonicalValue::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(CanonicalValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(CanonicalValue::Bool(false))
            }
            Some(b'"') => CanonicalText::new(self.parse_string()?).map(CanonicalValue::Text),
            Some(b'[') => self.parse_array(depth.saturating_add(1)),
            Some(b'{') => self.parse_object(depth.saturating_add(1)),
            Some(b'-' | b'0'..=b'9') => self.parse_integer(),
            _ => Err(ContractError::malformed("canonical_json")),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<CanonicalValue, ContractError> {
        self.consume(b'[')?;
        let mut values = Vec::new();
        if self.peek() == Some(b']') {
            self.position = self.position.saturating_add(1);
            return BoundedList::new(values).map(CanonicalValue::Array);
        }
        loop {
            if values.len() >= MAX_COLLECTION_ITEMS {
                return Err(ContractError::oversize(
                    "canonical_array",
                    MAX_COLLECTION_ITEMS,
                    values.len().saturating_add(1),
                ));
            }
            values.push(self.parse_value(depth)?);
            match self.peek() {
                Some(b',') => self.position = self.position.saturating_add(1),
                Some(b']') => {
                    self.position = self.position.saturating_add(1);
                    break;
                }
                _ => return Err(ContractError::malformed("canonical_array")),
            }
        }
        BoundedList::new(values).map(CanonicalValue::Array)
    }

    fn parse_object(&mut self, depth: usize) -> Result<CanonicalValue, ContractError> {
        self.consume(b'{')?;
        let mut entries = Vec::new();
        if self.peek() == Some(b'}') {
            self.position = self.position.saturating_add(1);
            return BoundedMap::from_entries(entries).map(CanonicalValue::Object);
        }
        loop {
            if entries.len() >= MAX_MAP_ENTRIES {
                return Err(ContractError::oversize(
                    "canonical_object",
                    MAX_MAP_ENTRIES,
                    entries.len().saturating_add(1),
                ));
            }
            let key = CanonicalKey::new_non_empty(self.parse_string()?)?;
            self.consume(b':')?;
            let item = self.parse_value(depth)?;
            entries.push((key, item));
            match self.peek() {
                Some(b',') => self.position = self.position.saturating_add(1),
                Some(b'}') => {
                    self.position = self.position.saturating_add(1);
                    break;
                }
                _ => return Err(ContractError::malformed("canonical_object")),
            }
        }
        if entries.len() == 1 && entries[0].0.as_str() == "$bytes" {
            let (_, value) = entries.pop().expect("length checked");
            if let CanonicalValue::Text(encoded) = value {
                return BoundedBytes::new(base64url_decode(encoded.as_str())?)
                    .map(CanonicalValue::Bytes);
            }
            return Err(ContractError::malformed("canonical_bytes"));
        }
        BoundedMap::from_entries(entries).map(CanonicalValue::Object)
    }

    fn parse_integer(&mut self) -> Result<CanonicalValue, ContractError> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position = self.position.saturating_add(1);
        }
        match self.peek() {
            Some(b'0') => {
                self.position = self.position.saturating_add(1);
                if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    return Err(ContractError::new(
                        ContractErrorKind::NonCanonical,
                        "canonical_integer",
                    ));
                }
            }
            Some(b'1'..=b'9') => {
                while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.position = self.position.saturating_add(1);
                }
            }
            _ => return Err(ContractError::malformed("canonical_integer")),
        }
        let text = core::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| ContractError::malformed("canonical_integer"))?;
        if text.starts_with('-') {
            let value = text.parse::<i64>().map_err(|_| {
                ContractError::new(ContractErrorKind::InvalidRange, "canonical_integer")
            })?;
            if value >= 0 {
                return Err(ContractError::new(
                    ContractErrorKind::NonCanonical,
                    "canonical_integer",
                ));
            }
            Ok(CanonicalValue::I64(value))
        } else {
            text.parse::<u64>().map(CanonicalValue::U64).map_err(|_| {
                ContractError::new(ContractErrorKind::InvalidRange, "canonical_integer")
            })
        }
    }

    fn parse_string(&mut self) -> Result<String, ContractError> {
        self.consume(b'"')?;
        let mut output = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| ContractError::malformed("canonical_string"))?;
            match byte {
                b'"' => {
                    self.position = self.position.saturating_add(1);
                    return Ok(output);
                }
                b'\\' => {
                    self.position = self.position.saturating_add(1);
                    self.parse_escape(&mut output)?;
                }
                0x00..=0x1f => {
                    return Err(ContractError::malformed("canonical_string"));
                }
                _ => {
                    let remaining = &self.bytes[self.position..];
                    let text = core::str::from_utf8(remaining)
                        .map_err(|_| ContractError::malformed("canonical_string"))?;
                    let character = text
                        .chars()
                        .next()
                        .ok_or_else(|| ContractError::malformed("canonical_string"))?;
                    output.push(character);
                    self.position = self.position.saturating_add(character.len_utf8());
                }
            }
            if output.len() > MAX_RAW_BYTES {
                return Err(ContractError::oversize(
                    "canonical_string",
                    MAX_RAW_BYTES,
                    output.len(),
                ));
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), ContractError> {
        let escaped = self
            .peek()
            .ok_or_else(|| ContractError::malformed("canonical_string_escape"))?;
        self.position = self.position.saturating_add(1);
        match escaped {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'b' => output.push('\u{08}'),
            b'f' => output.push('\u{0c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let first = self.parse_hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    self.consume(b'\\')?;
                    self.consume(b'u')?;
                    let second = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(ContractError::malformed("canonical_string_surrogate"));
                    }
                    0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(ContractError::malformed("canonical_string_surrogate"));
                } else {
                    u32::from(first)
                };
                output.push(
                    char::from_u32(scalar)
                        .ok_or_else(|| ContractError::malformed("canonical_string_scalar"))?,
                );
            }
            _ => return Err(ContractError::malformed("canonical_string_escape")),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, ContractError> {
        let end = self.position.saturating_add(4);
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| ContractError::malformed("canonical_string_escape"))?;
        let mut value = 0_u16;
        for byte in bytes {
            value = value
                .checked_mul(16)
                .and_then(|current| decode_hex(*byte).map(|nibble| current + u16::from(nibble)))
                .ok_or_else(|| ContractError::malformed("canonical_string_escape"))?;
        }
        self.position = end;
        Ok(value)
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), ContractError> {
        if self
            .bytes
            .get(self.position..self.position.saturating_add(literal.len()))
            == Some(literal)
        {
            self.position = self.position.saturating_add(literal.len());
            Ok(())
        } else {
            Err(ContractError::malformed("canonical_json"))
        }
    }

    fn consume(&mut self, expected: u8) -> Result<(), ContractError> {
        if self.peek() == Some(expected) {
            self.position = self.position.saturating_add(1);
            Ok(())
        } else {
            Err(ContractError::malformed("canonical_json"))
        }
    }

    fn check_depth(depth: usize) -> Result<(), ContractError> {
        if depth > MAX_CANONICAL_DEPTH {
            return Err(ContractError::bounded(
                ContractErrorKind::DepthExceeded,
                "canonical_json",
                usize_to_u64(MAX_CANONICAL_DEPTH),
                usize_to_u64(depth),
            ));
        }
        Ok(())
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }
}

struct CborParser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> CborParser<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn parse_value(&mut self, depth: usize) -> Result<CanonicalValue, ContractError> {
        if depth > MAX_CANONICAL_DEPTH {
            return Err(ContractError::bounded(
                ContractErrorKind::DepthExceeded,
                "canonical_cbor",
                usize_to_u64(MAX_CANONICAL_DEPTH),
                usize_to_u64(depth),
            ));
        }
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1f;
        match major {
            0 => Ok(CanonicalValue::U64(self.read_argument(additional)?)),
            1 => {
                let argument = self.read_argument(additional)?;
                let value = -1_i128 - i128::from(argument);
                i64::try_from(value).map(CanonicalValue::I64).map_err(|_| {
                    ContractError::new(ContractErrorKind::InvalidRange, "canonical_cbor")
                })
            }
            2 => {
                let length = self.read_length(additional, MAX_RAW_BYTES, "canonical_cbor_bytes")?;
                let value = self.read_exact(length)?.to_vec();
                BoundedBytes::new(value).map(CanonicalValue::Bytes)
            }
            3 => {
                let length = self.read_length(additional, MAX_RAW_BYTES, "canonical_cbor_text")?;
                let value = core::str::from_utf8(self.read_exact(length)?)
                    .map_err(|_| ContractError::malformed("canonical_cbor_text"))?
                    .to_owned();
                CanonicalText::new(value).map(CanonicalValue::Text)
            }
            4 => {
                let length =
                    self.read_length(additional, MAX_COLLECTION_ITEMS, "canonical_cbor_array")?;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.parse_value(depth.saturating_add(1))?);
                }
                BoundedList::new(values).map(CanonicalValue::Array)
            }
            5 => {
                let length =
                    self.read_length(additional, MAX_MAP_ENTRIES, "canonical_cbor_object")?;
                let mut entries = Vec::with_capacity(length);
                for _ in 0..length {
                    let key = match self.parse_value(depth.saturating_add(1))? {
                        CanonicalValue::Text(value) => {
                            CanonicalKey::new_non_empty(value.into_string())?
                        }
                        _ => return Err(ContractError::malformed("canonical_cbor_key")),
                    };
                    let value = self.parse_value(depth.saturating_add(1))?;
                    entries.push((key, value));
                }
                BoundedMap::from_entries(entries).map(CanonicalValue::Object)
            }
            7 if additional == 20 => Ok(CanonicalValue::Bool(false)),
            7 if additional == 21 => Ok(CanonicalValue::Bool(true)),
            7 if additional == 22 => Ok(CanonicalValue::Null),
            _ => Err(ContractError::malformed("canonical_cbor")),
        }
    }

    fn read_argument(&mut self, additional: u8) -> Result<u64, ContractError> {
        match additional {
            0..=23 => Ok(u64::from(additional)),
            24 => {
                let value = u64::from(self.read_byte()?);
                if value < 24 {
                    return Err(ContractError::new(
                        ContractErrorKind::NonCanonical,
                        "canonical_cbor_integer",
                    ));
                }
                Ok(value)
            }
            25 => {
                let bytes: [u8; 2] = self
                    .read_exact(2)?
                    .try_into()
                    .map_err(|_| ContractError::malformed("canonical_cbor_integer"))?;
                let value = u64::from(u16::from_be_bytes(bytes));
                if value <= 0xff {
                    return Err(ContractError::new(
                        ContractErrorKind::NonCanonical,
                        "canonical_cbor_integer",
                    ));
                }
                Ok(value)
            }
            26 => {
                let bytes: [u8; 4] = self
                    .read_exact(4)?
                    .try_into()
                    .map_err(|_| ContractError::malformed("canonical_cbor_integer"))?;
                let value = u64::from(u32::from_be_bytes(bytes));
                if value <= 0xffff {
                    return Err(ContractError::new(
                        ContractErrorKind::NonCanonical,
                        "canonical_cbor_integer",
                    ));
                }
                Ok(value)
            }
            27 => {
                let bytes: [u8; 8] = self
                    .read_exact(8)?
                    .try_into()
                    .map_err(|_| ContractError::malformed("canonical_cbor_integer"))?;
                let value = u64::from_be_bytes(bytes);
                if value <= 0xffff_ffff {
                    return Err(ContractError::new(
                        ContractErrorKind::NonCanonical,
                        "canonical_cbor_integer",
                    ));
                }
                Ok(value)
            }
            _ => Err(ContractError::malformed("canonical_cbor_argument")),
        }
    }

    fn read_length(
        &mut self,
        additional: u8,
        limit: usize,
        field: &'static str,
    ) -> Result<usize, ContractError> {
        let value = self.read_argument(additional)?;
        let length = usize::try_from(value)
            .map_err(|_| ContractError::new(ContractErrorKind::InvalidRange, field))?;
        if length > limit {
            return Err(ContractError::oversize(field, limit, length));
        }
        Ok(length)
    }

    fn read_byte(&mut self) -> Result<u8, ContractError> {
        let value = self
            .bytes
            .get(self.position)
            .copied()
            .ok_or_else(|| ContractError::malformed("canonical_cbor"))?;
        self.position = self.position.saturating_add(1);
        Ok(value)
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], ContractError> {
        let end = self.position.saturating_add(length);
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| ContractError::malformed("canonical_cbor"))?;
        self.position = end;
        Ok(value)
    }
}

pub(crate) fn base64url_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(TABLE[usize::from(first >> 2)]));
        output.push(char::from(
            TABLE[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                TABLE[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        }
        if chunk.len() > 2 {
            output.push(char::from(TABLE[usize::from(third & 0x3f)]));
        }
    }
    output
}

pub(crate) fn base64url_decode(input: &str) -> Result<Vec<u8>, ContractError> {
    if input.len() % 4 == 1 || input.bytes().any(|byte| byte == b'=') {
        return Err(ContractError::new(
            ContractErrorKind::InvalidToken,
            "base64url",
        ));
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3 + 2);
    for chunk in input.as_bytes().chunks(4) {
        let mut values = [0_u8; 4];
        for (index, byte) in chunk.iter().copied().enumerate() {
            values[index] = decode_base64url_byte(byte).ok_or_else(|| {
                ContractError::new(ContractErrorKind::InvalidCharacter, "base64url")
            })?;
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if chunk.len() > 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if chunk.len() > 3 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    if base64url_encode(&output) != input {
        return Err(ContractError::new(
            ContractErrorKind::NonCanonical,
            "base64url",
        ));
    }
    Ok(output)
}

fn decode_base64url_byte(value: u8) -> Option<u8> {
    match value {
        b'A'..=b'Z' => Some(value - b'A'),
        b'a'..=b'z' => Some(value - b'a' + 26),
        b'0'..=b'9' => Some(value - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(crate) fn hex_decode<const SIZE: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; SIZE], ContractError> {
    if value.len() != SIZE.saturating_mul(2) {
        return Err(ContractError::bounded(
            ContractErrorKind::InvalidDigest,
            field,
            usize_to_u64(SIZE.saturating_mul(2)),
            usize_to_u64(value.len()),
        ));
    }
    let mut output = [0_u8; SIZE];
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    debug_assert!(remainder.is_empty());
    for (index, pair) in pairs.iter().enumerate() {
        let high = decode_hex(pair[0])
            .ok_or_else(|| ContractError::new(ContractErrorKind::InvalidCharacter, field))?;
        let low = decode_hex(pair[1])
            .ok_or_else(|| ContractError::new(ContractErrorKind::InvalidCharacter, field))?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn decode_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

/// RFC 3339 UTC timestamp with exactly six fractional digits.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UtcTimestamp(String);

impl UtcTimestamp {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let bytes = value.as_bytes();
        let separators = [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (26, b'Z'),
        ];
        if bytes.len() != 27
            || separators
                .into_iter()
                .any(|(index, expected)| bytes.get(index) != Some(&expected))
            || bytes.iter().enumerate().any(|(index, byte)| {
                !matches!(index, 4 | 7 | 10 | 13 | 16 | 19 | 26) && !byte.is_ascii_digit()
            })
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidCharacter,
                "utc_timestamp",
            ));
        }
        let parse = |range: core::ops::Range<usize>| -> Result<u32, ContractError> {
            value[range]
                .parse()
                .map_err(|_| ContractError::malformed("utc_timestamp"))
        };
        let year = parse(0..4)?;
        let month = parse(5..7)?;
        let day = parse(8..10)?;
        let hour = parse(11..13)?;
        let minute = parse(14..16)?;
        let second = parse(17..19)?;
        if year == 0
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "utc_timestamp",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DurationMillis(u64);

impl DurationMillis {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeadlineMillis(u64);

impl DeadlineMillis {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
