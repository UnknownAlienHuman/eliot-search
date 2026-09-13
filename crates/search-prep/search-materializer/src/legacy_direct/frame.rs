//! Exact persisted legacy DIRECT preparation frame codec.

use core::fmt;

/// Closed legacy preparation framing failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyDirectPreparationError {
    /// The frame tag, cardinality or layout body is invalid.
    InvalidFrame,
}

impl LegacyDirectPreparationError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidFrame => "DIRECT_PREPARATION_INVALID",
        }
    }
}

impl fmt::Display for LegacyDirectPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyDirectPreparationError {}

/// Exact closed gap encoded by the legacy DIRECT preparation frame.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyDirectPreparationGap {
    /// Retained bytes are not UTF-8.
    RevisionNotUtf8,
    /// Retained bytes violate the text/binary policy.
    BinaryContent,
    /// Exact materialization exceeded its line ceiling.
    TooManyLines,
    /// Exact unitization exceeded its unit ceiling.
    TooManyUnits,
    /// Encoded layout exceeded the retained-object ceiling.
    LayoutTooLarge,
    /// A leading BOM would shift exact DIRECT coordinates.
    RevisionHasBom,
}

impl LegacyDirectPreparationGap {
    /// Persisted singleton tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::RevisionNotUtf8 => 1,
            Self::BinaryContent => 2,
            Self::TooManyLines => 3,
            Self::TooManyUnits => 4,
            Self::LayoutTooLarge => 5,
            Self::RevisionHasBom => 6,
        }
    }

    /// Stable daemon-compatible gap reason.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::RevisionNotUtf8 => "DIRECT_REVISION_NOT_UTF8",
            Self::BinaryContent => "MATERIALIZATION_BINARY_CONTENT",
            Self::TooManyLines => "MATERIALIZATION_TOO_MANY_LINES",
            Self::TooManyUnits => "UNITIZATION_TOO_MANY_UNITS",
            Self::LayoutTooLarge => "DIRECT_PREPARATION_LAYOUT_TOO_LARGE",
            Self::RevisionHasBom => "DIRECT_REVISION_HAS_BOM",
        }
    }
}

/// Borrowed decoded legacy DIRECT preparation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDirectPreparationFrame<'a> {
    /// Non-empty exact unit layout bytes.
    Layout(&'a [u8]),
    /// Closed deterministic preparation gap.
    Gap(LegacyDirectPreparationGap),
}

impl<'a> LegacyDirectPreparationFrame<'a> {
    /// Identity marker used by representation derivation.
    ///
    /// Layouts bind their exact bytes; gaps bind their stable reason text.
    #[must_use]
    pub const fn identity_marker(self) -> &'a [u8] {
        match self {
            Self::Layout(layout) => layout,
            Self::Gap(gap) => gap.reason().as_bytes(),
        }
    }

    /// Gap reason, or `None` for a searchable layout.
    #[must_use]
    pub const fn gap_reason(self) -> Option<&'static str> {
        match self {
            Self::Layout(_) => None,
            Self::Gap(gap) => Some(gap.reason()),
        }
    }
}

/// Decodes the exact persisted legacy DIRECT preparation frame.
///
/// Layout tag `0` requires a non-empty body. Gap tags are fixed singletons;
/// trailing bytes and unknown tags fail closed.
pub const fn decode_legacy_direct_preparation(
    encoded: &[u8],
) -> Result<LegacyDirectPreparationFrame<'_>, LegacyDirectPreparationError> {
    match encoded {
        [0, layout @ ..] if !layout.is_empty() => {
            Ok(LegacyDirectPreparationFrame::Layout(layout))
        }
        [1] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::RevisionNotUtf8,
        )),
        [2] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::BinaryContent,
        )),
        [3] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::TooManyLines,
        )),
        [4] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::TooManyUnits,
        )),
        [5] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::LayoutTooLarge,
        )),
        [6] => Ok(LegacyDirectPreparationFrame::Gap(
            LegacyDirectPreparationGap::RevisionHasBom,
        )),
        _ => Err(LegacyDirectPreparationError::InvalidFrame),
    }
}

/// Encodes a non-empty exact layout under the version-one layout tag.
pub fn encode_legacy_direct_layout(
    layout: &[u8],
) -> Result<Vec<u8>, LegacyDirectPreparationError> {
    if layout.is_empty() {
        return Err(LegacyDirectPreparationError::InvalidFrame);
    }
    let mut encoded = Vec::with_capacity(layout.len().saturating_add(1));
    encoded.push(0);
    encoded.extend_from_slice(layout);
    Ok(encoded)
}

/// Encodes one closed gap as its exact singleton frame.
#[must_use]
pub fn encode_legacy_direct_gap(gap: LegacyDirectPreparationGap) -> Vec<u8> {
    vec![gap.tag()]
}
