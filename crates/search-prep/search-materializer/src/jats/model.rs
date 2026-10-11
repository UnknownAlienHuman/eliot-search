//! Closed flat observations, provenance and positive finite dimensions.
use std::fmt;

macro_rules! limits {
    ($($field:ident = $cap:expr),+ $(,)?) => {
        /// Positive finite owner limits, independently lowerable by an operation.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct JatsLimits {
            $(#[doc = concat!("Maximum admitted ", stringify!($field), ".")]
            pub $field: usize,)+
        }
        pub(super) const HARD: JatsLimits = JatsLimits { $($field: $cap,)+ };
        impl JatsLimits {
            pub(super) const fn dimensions(self) -> [usize; 19] { [$(self.$field,)+] }
            pub(super) fn narrow(self, ceiling: Self) -> Result<Self, JatsError> {
                if self.dimensions().into_iter().zip(ceiling.dimensions())
                    .any(|(value, max)| value == 0 || value > max) {
                    return Err(JatsError::InvalidLimits);
                }
                Ok(self)
            }
        }
    };
}
limits! {
    input_bytes = 1024 * 1024, event_bytes = 16 * 1024, events = 65_536,
    depth = 64, elements = 2048, attributes_each = 32, attributes = 8192,
    namespaces = 2, mapped_bytes_each = 16 * 1024, mapped_bytes = 512 * 1024,
    texts = 8192, comments = 1024, sections = 256, figures = 128, tables = 128,
    references = 512, owned_bytes = 4 * 1024 * 1024, work_steps = 16 * 1024 * 1024,
    attribute_bytes = 4096,
}

/// Content-free closed refusal; donor/source messages are never exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JatsError {
    /// A zero, widened or unsupported limit.
    InvalidLimits,
    /// Original cancellation flag observed.
    Cancelled,
    /// Original absolute deadline elapsed.
    DeadlineExceeded,
    /// A positive finite input/event/count/storage/work/byte ceiling was exceeded.
    LimitExceeded,
    /// Fallible owner allocation refused.
    AllocationFailed,
    /// Checked arithmetic overflow.
    Overflow,
    /// XML parser or document pairing refused.
    MalformedXml,
    /// Character is outside XML 1.0.
    InvalidCharacter,
    /// DTD, PI, encoding, entity, element, attribute or namespace is unsupported.
    UnsupportedSyntax,
    /// Original source extent is invalid.
    InvalidRange,
    /// Request/raw/structural profiles disagree.
    ProfileMismatch,
    /// Owning raw pipeline refused.
    Materialization(crate::MaterializationError),
    /// Shared canonical commitment refused.
    CanonicalEncoding,
    /// Complete owning reconstruction differs from the proposal.
    ProductMismatch,
}

macro_rules! names {
    ($name:ident { $($variant:ident = $literal:literal),+ $(,)? }) => {
        #[doc = concat!("Closed observed ", stringify!($name), " values.")]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $(#[doc = concat!("Observed inert ", $literal, " syntax.")]
            $variant,)+
        }
        impl $name {
            pub(super) fn from_name(name: &str) -> Result<Self, JatsError> {
                match name { $($literal => Ok(Self::$variant),)+ _ => Err(JatsError::UnsupportedSyntax) }
            }
            /// Frozen literal syntax name, not an external identity.
            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $literal,)+ }
            }
        }
    };
}
names! { JatsElementKind {
    Article = "article", Front = "front", ArticleMeta = "article-meta",
    JournalMeta = "journal-meta", JournalId = "journal-id", Issn = "issn",
    TitleGroup = "title-group", ArticleTitle = "article-title", Subtitle = "subtitle",
    Abstract = "abstract", Body = "body", Section = "sec", Title = "title",
    Paragraph = "p", List = "list", ListItem = "list-item", BoxedText = "boxed-text",
    Quote = "disp-quote", Code = "code", Preformat = "preformat", Figure = "fig",
    Caption = "caption", TableWrap = "table-wrap", Table = "table", TableHead = "thead",
    TableBody = "tbody", Row = "tr", HeaderCell = "th", Cell = "td",
    ReferenceList = "ref-list", Reference = "ref", CrossReference = "xref",
    ExternalLink = "ext-link", Back = "back", Acknowledgment = "ack",
    FootnoteGroup = "fn-group", Footnote = "fn", Italic = "italic", Bold = "bold",
    Superscript = "sup", Subscript = "sub", Monospace = "monospace",
    Underline = "underline", NamedContent = "named-content", Label = "label",
    MixedCitation = "mixed-citation", ElementCitation = "element-citation",
} }
names! { JatsAttributeKind {
    Id = "id", Rid = "rid", RefType = "ref-type", Href = "xlink:href",
    DtdVersion = "dtd-version", ArticleType = "article-type", Lang = "xml:lang",
    Space = "xml:space", SectionType = "sec-type", ListType = "list-type",
    ContentType = "content-type", SpecificUse = "specific-use", Position = "position",
    Orientation = "orientation", Colspan = "colspan", Rowspan = "rowspan",
    Align = "align", Valign = "valign", Scope = "scope", Headers = "headers",
    PublicationType = "publication-type", XlinkType = "xlink:type",
    XlinkNamespace = "xmlns:xlink", XmlNamespace = "xmlns:xml",
    JournalIdType = "journal-id-type", PublicationFormat = "publication-format",
} }

/// Original retained UTF8 half-open byte extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}
impl JatsSpan {
    /// Original inclusive byte start.
    pub const fn start(self) -> usize {
        self.start
    }
    /// Original exclusive byte end.
    pub const fn end(self) -> usize {
        self.end
    }
}
/// Complete supported event inventory; full JATS schema validity is unclaimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JatsObservationStatus {
    /// Closed subset, mapped values with whole-event provenance, raw exact XML.
    CompleteSubsetMappedValues,
}
/// Immutable observed element with original extent and physical parent.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsElement {
    pub(super) kind: JatsElementKind,
    pub(super) parent: Option<usize>,
    pub(super) event: usize,
    pub(super) close_event: usize,
    pub(super) opening: JatsSpan,
    pub(super) closing: Option<JatsSpan>,
    pub(super) span: JatsSpan,
}
impl JatsElement {
    /// Closed observed element kind.
    pub const fn kind(&self) -> JatsElementKind {
        self.kind
    }
    /// Earlier containing element ordinal.
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }
    /// Opening event ordinal.
    pub const fn event(&self) -> usize {
        self.event
    }
    /// Closing event ordinal; opening ordinal for empty syntax.
    pub const fn close_event(&self) -> usize {
        self.close_event
    }
    /// Original opening syntax.
    pub const fn opening(&self) -> JatsSpan {
        self.opening
    }
    /// Original closing syntax, absent for empty elements.
    pub const fn closing(&self) -> Option<JatsSpan> {
        self.closing
    }
    /// Entire original element extent, including nested content.
    pub const fn span(&self) -> JatsSpan {
        self.span
    }
}
/// Mapped text observation form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JatsTextKind {
    /// XML1.0 newline-normalized text.
    Text,
    /// XML1.0 newline-normalized CDATA.
    Cdata,
    /// One predefined or numeric reference.
    Reference,
}
/// Decoded text with whole-event provenance; never an exact decoded subanchor.
#[derive(Eq, PartialEq)]
pub struct JatsText {
    pub(super) owner: usize,
    pub(super) event: usize,
    pub(super) span: JatsSpan,
    pub(super) kind: JatsTextKind,
    pub(super) value: String,
}
impl JatsText {
    /// Owning element ordinal.
    pub const fn owner(&self) -> usize {
        self.owner
    }
    /// Original event ordinal.
    pub const fn event(&self) -> usize {
        self.event
    }
    /// Original whole-event provenance extent.
    pub const fn span(&self) -> JatsSpan {
        self.span
    }
    /// Closed mapped form.
    pub const fn kind(&self) -> JatsTextKind {
        self.kind
    }
    /// Mapped decoded value; not exact evidence.
    pub fn value(&self) -> &str {
        &self.value
    }
}
impl fmt::Debug for JatsText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JatsText")
            .field("owner", &self.owner)
            .field("event", &self.event)
            .field("span", &self.span)
            .field("kind", &self.kind)
            .field("bytes", &self.value.len())
            .finish()
    }
}
/// Normalized inert attribute with whole-opening-tag provenance.
#[derive(Eq, PartialEq)]
pub struct JatsAttribute {
    pub(super) owner: usize,
    pub(super) kind: JatsAttributeKind,
    pub(super) span: JatsSpan,
    pub(super) value: String,
}
impl JatsAttribute {
    /// Owning element ordinal.
    pub const fn owner(&self) -> usize {
        self.owner
    }
    /// Closed name.
    pub const fn kind(&self) -> JatsAttributeKind {
        self.kind
    }
    /// Entire original opening tag; never an exact normalized-value subanchor.
    pub const fn span(&self) -> JatsSpan {
        self.span
    }
    /// Normalized inert value, never a fetched resource or verified scholarly ID.
    pub fn value(&self) -> &str {
        &self.value
    }
}
impl fmt::Debug for JatsAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JatsAttribute")
            .field("owner", &self.owner)
            .field("kind", &self.kind)
            .field("span", &self.span)
            .field("bytes", &self.value.len())
            .finish()
    }
}
/// Inert original comment extent; its content is never copied or executed.
#[derive(Debug, Eq, PartialEq)]
pub struct JatsComment {
    pub(super) owner: Option<usize>,
    pub(super) event: usize,
    pub(super) span: JatsSpan,
}
impl JatsComment {
    /// Owning element, absent for prolog and epilog comments.
    pub const fn owner(&self) -> Option<usize> {
        self.owner
    }
    /// Original event ordinal.
    pub const fn event(&self) -> usize {
        self.event
    }
    /// Inert original whole-comment syntax extent.
    pub const fn span(&self) -> JatsSpan {
        self.span
    }
}
#[derive(Default, Eq, PartialEq)]
pub(super) struct Inventory {
    pub(super) elements: Vec<JatsElement>,
    pub(super) attributes: Vec<JatsAttribute>,
    pub(super) texts: Vec<JatsText>,
    pub(super) comments: Vec<JatsComment>,
    pub(super) events: usize,
    pub(super) mapped: usize,
    pub(super) namespaces: usize,
    pub(super) declaration: Option<JatsSpan>,
}
impl Inventory {
    pub(super) fn count(&self) -> Result<usize, JatsError> {
        self.elements
            .len()
            .checked_add(self.attributes.len())
            .and_then(|v| v.checked_add(self.texts.len()))
            .and_then(|v| v.checked_add(self.comments.len()))
            .ok_or(JatsError::Overflow)
    }
}
