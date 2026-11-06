use crate::{ContractError, ContractErrorKind};
use core::{fmt, str::FromStr};

/// Canonical 16-byte UUID storage shared by strongly typed identifiers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct UuidBytes([u8; 16]);

impl UuidBytes {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        let bytes = value.as_bytes();
        if bytes.len() != 36
            || bytes.get(8) != Some(&b'-')
            || bytes.get(13) != Some(&b'-')
            || bytes.get(18) != Some(&b'-')
            || bytes.get(23) != Some(&b'-')
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidCharacter,
                "uuid",
            ));
        }
        let mut compact = [0_u8; 32];
        let mut index = 0;
        for (position, byte) in bytes.iter().copied().enumerate() {
            if matches!(position, 8 | 13 | 18 | 23) {
                continue;
            }
            if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte) {
                return Err(ContractError::new(
                    ContractErrorKind::InvalidCharacter,
                    "uuid",
                ));
            }
            compact[index] = byte;
            index = index.saturating_add(1);
        }
        let mut output = [0_u8; 16];
        let (pairs, remainder) = compact.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        for (output_index, pair) in pairs.iter().enumerate() {
            let high = lower_hex(pair[0]).ok_or_else(|| ContractError::malformed("uuid"))?;
            let low = lower_hex(pair[1]).ok_or_else(|| ContractError::malformed("uuid"))?;
            output[output_index] = (high << 4) | low;
        }
        Ok(Self(output))
    }
}

impl fmt::Display for UuidBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = hex_encode(&self.0);
        write!(
            formatter,
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

impl FromStr for UuidBytes {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

const fn lower_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

macro_rules! uuid_newtype {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(UuidBytes);

            impl $name {
                #[must_use]
                pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                    Self(UuidBytes::from_bytes(bytes))
                }

                #[must_use]
                pub const fn as_bytes(&self) -> &[u8; 16] {
                    self.0.as_bytes()
                }

                /// Parses the canonical lowercase UUID spelling.
                ///
                /// # Errors
                /// Returns a content-free error for a noncanonical UUID.
                pub fn parse(value: &str) -> Result<Self, ContractError> {
                    UuidBytes::parse(value).map(Self)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(formatter)
                }
            }

            impl FromStr for $name {
                type Err = ContractError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Self::parse(value)
                }
            }
        )+
    };
}

uuid_newtype!(SourceNamespaceId, SourceId);

/// BLAKE3 digest bytes shared by immutable object references.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Blake3Digest32([u8; 32]);

impl Blake3Digest32 {
    /// Wraps stored or independently computed bytes; this performs no hashing.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Blake3Digest32 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex_encode(&self.0))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
