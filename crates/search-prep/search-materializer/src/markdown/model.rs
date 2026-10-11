//! Closed raw-range Markdown observations and finite owner limits.

use std::fmt;

/// Raw adapter ceilings; lower operation limits must validate against these.
pub(super) const HARD_LIMITS: MarkdownLimits = MarkdownLimits {
    input_bytes: 8 * 1024 * 1024,
    next_attempts: 131_072,
    stack_depth: 128,
    nodes: 4096,
    inline_records: 4096,
    metadata_bytes_each: 4096,
    metadata_bytes_total: 1024 * 1024,
    owned_bytes: 4 * 1024 * 1024,
    work_steps: 16 * 1024 * 1024,
};

/// Positive finite ceilings for the additional Markdown pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownLimits {
    /// Maximum exact raw UTF-8 input bytes.
    pub input_bytes: usize,
    /// Maximum donor next calls, including the terminal call returning None.
    pub next_attempts: usize,
    /// Maximum active parser-container frames.
    pub stack_depth: usize,
    /// Maximum structural observations, including the root.
    pub nodes: usize,
    /// Maximum inline observations.
    pub inline_records: usize,
    /// Maximum bytes in one retained mapped metadata value.
    pub metadata_bytes_each: usize,
    /// Maximum cumulative bytes in retained metadata values.
    pub metadata_bytes_total: usize,
    /// Logical record/frame/string storage; excludes donor internals and raw maps.
    pub owned_bytes: usize,
    /// Maximum admitted parser, identity and comparison work units.
    pub work_steps: usize,
}

pub(super) struct ValidatedLimits(MarkdownLimits);

impl MarkdownLimits {
    pub(super) fn validate(self) -> Result<ValidatedLimits, MarkdownError> {
        let dimensions = [
            (self.input_bytes, HARD_LIMITS.input_bytes),
            (self.next_attempts, HARD_LIMITS.next_attempts),
            (self.stack_depth, HARD_LIMITS.stack_depth),
            (self.nodes, HARD_LIMITS.nodes),
            (self.inline_records, HARD_LIMITS.inline_records),
            (self.metadata_bytes_each, HARD_LIMITS.metadata_bytes_each),
            (self.metadata_bytes_total, HARD_LIMITS.metadata_bytes_total),
            (self.owned_bytes, HARD_LIMITS.owned_bytes),
            (self.work_steps, HARD_LIMITS.work_steps),
        ];
        if dimensions
            .iter()
            .any(|&(value, hard)| value == 0 || value > hard)
        {
            return Err(MarkdownError::InvalidLimits);
        }
        Ok(ValidatedLimits(self))
    }
}

impl ValidatedLimits {
    pub(super) const fn get(&self) -> MarkdownLimits {
        self.0
    }
}

/// No source, metadata, vendor error text or path is carried by refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownError {
    /// A limit is zero or exceeds its accepted ceiling.
    InvalidLimits,
    /// Original cancellation token was observed.
    Cancelled,
    /// Original monotonic deadline elapsed at a checkpoint.
    DeadlineExceeded,
    /// Nonempty raw source is required.
    EmptyInput,
    /// A leading BOM is outside this raw profile.
    BomNotSupported,
    /// Source is not strict UTF-8.
    InvalidUtf8,
    /// Input exceeds the effective byte ceiling.
    InputLimit,
    /// Another iterator call would exceed the attempt ceiling.
    AttemptLimit,
    /// Active parser nesting exceeds its ceiling.
    StackLimit,
    /// Structural inventory exceeds its ceiling.
    NodeLimit,
    /// Inline inventory exceeds its ceiling.
    InlineLimit,
    /// Selected mapped metadata exceeds an item or aggregate ceiling.
    MetadataLimit,
    /// Checked logical storage exceeds its ceiling.
    StorageLimit,
    /// Admitted work exceeds its ceiling.
    WorkLimit,
    /// A fallible owner reservation failed; donor allocation is separate.
    AllocationFailed,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// Donor range is outside original UTF-8 coordinates.
    InvalidRange,
    /// Container pairing, parent or record ordering is inconsistent.
    InvalidStructure,
    /// An event outside the closed profile was encountered.
    UnsupportedEvent,
    /// Existing exact raw materialization refused.
    Materialization(crate::MaterializationError),
    /// Request, context and Markdown raw profile do not agree.
    ProfileMismatch,
    /// Shared canonical construction or byte admission refused.
    CanonicalEncoding,
    /// Complete source replay differs from the proposed product.
    ProductMismatch,
}

/// Checked original-source half-open UTF-8 byte range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl RawSpan {
    /// Inclusive original byte start.
    pub const fn start(self) -> usize {
        self.start
    }
    /// Exclusive original byte end.
    pub const fn end(self) -> usize {
        self.end
    }
    pub(super) const fn contains(self, child: Self) -> bool {
        self.start <= child.start && child.end <= self.end
    }
}

/// Assurance of a retained decoded syntax value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataAssurance {
    /// Value is an observation with whole-event provenance, not an exact subrange.
    MappedText,
}

/// Decoded donor value, with no claimed exact subrange within its event span.
#[derive(Eq, PartialEq)]
pub struct MappedText {
    pub(super) value: String,
    pub(super) assurance: MetadataAssurance,
}

impl MappedText {
    /// Bounded decoded syntax observation; no render or fetch is implied.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Assurance ceiling for this observed value.
    pub const fn assurance(&self) -> MetadataAssurance {
        self.assurance
    }
}

impl fmt::Debug for MappedText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedText")
            .field("bytes", &self.value.len())
            .field("assurance", &self.assurance)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Closed structural observations from the selected `CommonMark` profile.
pub enum MarkdownKind {
    /// Raw root containing every original byte.
    Root,
    /// Heading syntax with a separate global logical hierarchy.
    Heading {
        /// Source heading level from one through six.
        level: u8,
        /// Earlier lower-level heading ordinal, independent of containment.
        parent_section: Option<usize>,
    },
    /// Paragraph event emitted by the donor; tight-list omissions are preserved.
    Paragraph,
    /// List item with the nearest structural physical parent.
    Item,
    /// Block quote.
    Quote,
    /// Fenced code block with selected inert info metadata.
    FencedCode,
    /// Indented code block.
    IndentedCode,
    /// Thematic break.
    ThematicBreak,
    /// Inert raw HTML block, never rendered.
    RawHtmlBlock,
}

#[derive(Debug, Eq, PartialEq)]
/// Immutable original-range structural observation; not a unit occurrence.
pub struct MarkdownNode {
    /// Index in document.nodes, assigned on entry in donor preorder.
    pub(super) ordinal: usize,
    pub(super) kind: MarkdownKind,
    pub(super) span: RawSpan,
    /// Nearest published structural container; transparent lists have no node.
    pub(super) physical_parent: Option<usize>,
    /// Independent logical heading section; a heading refers to itself here.
    pub(super) heading_section: Option<usize>,
    pub(super) code_info: Option<MappedText>,
}

impl MarkdownNode {
    /// Stable preorder ordinal within this exact result.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    /// Closed structural kind and heading hierarchy.
    pub const fn kind(&self) -> MarkdownKind {
        self.kind
    }
    /// Original byte extent, which may overlap child extents.
    pub const fn span(&self) -> RawSpan {
        self.span
    }
    /// Nearest containing published structural node.
    pub const fn physical_parent(&self) -> Option<usize> {
        self.physical_parent
    }
    /// Global logical heading ordinal; no physical containment is asserted.
    pub const fn heading_section(&self) -> Option<usize> {
        self.heading_section
    }
    /// Inert mapped fenced-code info observation.
    pub const fn code_info(&self) -> Option<&MappedText> {
        self.code_info.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Donor-observed inert reference form; completeness is separately limited.
pub enum ObservedLinkKind {
    /// Inline destination syntax.
    Inline,
    /// Explicit reference syntax.
    Reference,
    /// Collapsed reference syntax.
    Collapsed,
    /// Shortcut reference syntax.
    Shortcut,
    /// Automatic URI link syntax.
    Autolink,
    /// Automatic email syntax.
    Email,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Closed inline event observations with original whole-event provenance.
pub enum InlineKind {
    /// Emphasis container observation.
    Emphasis,
    /// Strong container observation.
    Strong,
    /// Selected mapped inline-code observation.
    Code,
    /// Text event span without a retained decoded payload.
    Text,
    /// Soft break event.
    SoftBreak,
    /// Hard break event.
    HardBreak,
    /// Inert HTML event span without a retained decoded payload.
    Html,
    /// Observed link; destination is never fetched.
    Link(ObservedLinkKind),
    /// Observed image syntax; destination is never fetched.
    Image(ObservedLinkKind),
}

#[derive(Debug, Eq, PartialEq)]
/// Immutable inline observation owned by a structural node.
pub struct InlineRecord {
    pub(super) owner: usize,
    /// The whole donor event span, not an exact anchor for a decoded value.
    pub(super) span: RawSpan,
    pub(super) kind: InlineKind,
    /// Only inline Code retains this mapped value; Text/Html are span-only.
    pub(super) text: Option<MappedText>,
    pub(super) destination: Option<MappedText>,
    pub(super) title: Option<MappedText>,
}

impl InlineRecord {
    /// Structural owner ordinal.
    pub const fn owner(&self) -> usize {
        self.owner
    }
    /// Whole original event range, including possible zero-width observations.
    pub const fn span(&self) -> RawSpan {
        self.span
    }
    /// Closed observed event form.
    pub const fn kind(&self) -> InlineKind {
        self.kind
    }
    /// Selected mapped inline-code value; Text/HTML payloads remain absent.
    pub const fn text(&self) -> Option<&MappedText> {
        self.text.as_ref()
    }
    /// Selected inert mapped destination.
    pub const fn destination(&self) -> Option<&MappedText> {
        self.destination.as_ref()
    }
    /// Selected inert mapped title.
    pub const fn title(&self) -> Option<&MappedText> {
        self.title.as_ref()
    }
}

/// Explicit limit of inline observation completeness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlineMetadataStatus {
    /// Every supported inline event has kind/span, but decoded Text/Html values
    /// are intentionally unproduced. Only Code and observed reference values
    /// are retained here; code info is retained on the structural node.
    /// This is not a complete decoded display-text or reference inventory.
    ObservedSpansAndSelectedValues,
}

/// Reference metadata is not an exhaustive link inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceMetadataStatus {
    /// Donor expansion fuel/fallback can leave references as ordinary text.
    PotentiallyIncomplete,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct MarkdownUsage {
    pub(super) next_attempts: usize,
    pub(super) yielded_events: usize,
    pub(super) steps: usize,
    pub(super) metadata_bytes: usize,
    pub(super) owned_live: usize,
    pub(super) owned_peak: usize,
}

/// Exact raw source is borrowed unchanged; nodes/metadata are flat records only.
/// This is neither a `UnitSet` nor an admitted/versioned representation.
pub(super) struct MarkdownDocument<'a> {
    pub(super) source: &'a str,
    pub(super) nodes: Vec<MarkdownNode>,
    pub(super) inline_records: Vec<InlineRecord>,
    pub(super) inline_metadata: InlineMetadataStatus,
    pub(super) reference_metadata: ReferenceMetadataStatus,
    pub(super) usage: MarkdownUsage,
}
