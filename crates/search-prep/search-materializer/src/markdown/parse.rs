//! Private raw-preserving pulldown-cmark 0.13.1 adapter.
//! Only empty Options are accepted; all raw bytes remain authoritative.
//!
//! Accounting: input bytes once before UTF-8 validation/eager construction;
//! each next attempt (including terminal None) is capped before the donor call;
//! yielded events are counted separately. Work charges each attempt/event/range
//! validation/containment, each record/stack operation and each copied metadata
//! byte. These are admitted-work units, not instrumentation of donor CPU steps.
//! Logical owned storage counts live records, frames and copied string lengths;
//! it excludes borrowed input, spare allocator capacity and donor internals.
//! Constructor/next are synchronous and not preemptible by this adapter.

use crate::request::CancellationToken;
use std::mem::size_of;
use std::ops::Range;
use std::time::Instant;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

use super::model::{
    InlineKind, InlineMetadataStatus, InlineRecord, MappedText, MarkdownDocument, MarkdownError,
    MarkdownKind, MarkdownLimits, MarkdownNode, MarkdownUsage, MetadataAssurance, ObservedLinkKind,
    RawSpan, ReferenceMetadataStatus,
};

struct Meter {
    limits: MarkdownLimits,
    used: MarkdownUsage,
}

impl Meter {
    fn steps(&mut self, amount: usize) -> Result<(), MarkdownError> {
        self.used.steps = add(
            self.used.steps,
            amount,
            self.limits.work_steps,
            MarkdownError::WorkLimit,
        )?;
        Ok(())
    }

    fn storage(&mut self, amount: usize) -> Result<(), MarkdownError> {
        self.used.owned_live = add(
            self.used.owned_live,
            amount,
            self.limits.owned_bytes,
            MarkdownError::StorageLimit,
        )?;
        self.used.owned_peak = self.used.owned_peak.max(self.used.owned_live);
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<MappedText, MarkdownError> {
        if text.len() > self.limits.metadata_bytes_each {
            return Err(MarkdownError::MetadataLimit);
        }
        self.used.metadata_bytes = add(
            self.used.metadata_bytes,
            text.len(),
            self.limits.metadata_bytes_total,
            MarkdownError::MetadataLimit,
        )?;
        self.storage(text.len())?;
        self.steps(text.len())?;
        let mut value = String::new();
        value
            .try_reserve_exact(text.len())
            .map_err(|_| MarkdownError::AllocationFailed)?;
        value.push_str(text);
        Ok(MappedText {
            value,
            assurance: MetadataAssurance::MappedText,
        })
    }
}

fn add(
    value: usize,
    amount: usize,
    cap: usize,
    error: MarkdownError,
) -> Result<usize, MarkdownError> {
    let next = value
        .checked_add(amount)
        .ok_or(MarkdownError::ArithmeticOverflow)?;
    if next > cap {
        return Err(error);
    }
    Ok(next)
}

pub(super) fn checkpoint(
    cancelled: CancellationToken<'_>,
    deadline: Instant,
) -> Result<(), MarkdownError> {
    if cancelled.is_cancelled() {
        return Err(MarkdownError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(MarkdownError::DeadlineExceeded);
    }
    Ok(())
}

struct Frame {
    expected: TagEnd,
    span: RawSpan,
    owner: usize,
}

struct Builder<'a> {
    source: &'a str,
    nodes: Vec<MarkdownNode>,
    inline_records: Vec<InlineRecord>,
    stack: Vec<Frame>,
    headings: [Option<usize>; 6],
    section: Option<usize>,
    meter: Meter,
}

impl Builder<'_> {
    fn owner(&self) -> usize {
        self.stack.last().map_or(0, |frame| frame.owner)
    }

    fn span(&mut self, range: Range<usize>) -> Result<RawSpan, MarkdownError> {
        self.meter.steps(4)?;
        if range.start > range.end
            || range.end > self.source.len()
            || !self.source.is_char_boundary(range.start)
            || !self.source.is_char_boundary(range.end)
        {
            return Err(MarkdownError::InvalidRange);
        }
        let span = RawSpan {
            start: range.start,
            end: range.end,
        };
        self.meter.steps(1)?;
        let container = self.stack.last().map_or(
            RawSpan {
                start: 0,
                end: self.source.len(),
            },
            |frame| frame.span,
        );
        if !container.contains(span) {
            return Err(MarkdownError::InvalidStructure);
        }
        Ok(span)
    }

    fn node(
        &mut self,
        kind: MarkdownKind,
        span: RawSpan,
        info: Option<&str>,
    ) -> Result<usize, MarkdownError> {
        add(
            self.nodes.len(),
            1,
            self.meter.limits.nodes,
            MarkdownError::NodeLimit,
        )?;
        if span.start == span.end
            || self
                .nodes
                .last()
                .is_some_and(|node| span.start < node.span.start)
        {
            return Err(MarkdownError::InvalidStructure);
        }
        let ordinal = self.nodes.len();
        let physical_parent = (ordinal != 0).then(|| self.owner());
        if let Some(parent) = physical_parent
            && !self
                .nodes
                .get(parent)
                .is_some_and(|node| node.span.contains(span))
        {
            return Err(MarkdownError::InvalidStructure);
        }
        self.meter.storage(size_of::<MarkdownNode>())?;
        self.meter.steps(1)?;
        let code_info = info.map(|value| self.meter.text(value)).transpose()?;
        self.nodes
            .try_reserve_exact(1)
            .map_err(|_| MarkdownError::AllocationFailed)?;
        self.nodes.push(MarkdownNode {
            ordinal,
            kind,
            span,
            physical_parent,
            heading_section: self.section,
            code_info,
        });
        Ok(ordinal)
    }

    fn inline(
        &mut self,
        kind: InlineKind,
        span: RawSpan,
        text: Option<&str>,
        reference: Option<(&str, &str)>,
    ) -> Result<(), MarkdownError> {
        add(
            self.inline_records.len(),
            1,
            self.meter.limits.inline_records,
            MarkdownError::InlineLimit,
        )?;
        self.meter.storage(size_of::<InlineRecord>())?;
        self.meter.steps(1)?;
        let owner = self.owner();
        if !self
            .nodes
            .get(owner)
            .is_some_and(|node| node.span.contains(span))
        {
            return Err(MarkdownError::InvalidStructure);
        }
        let text = text.map(|value| self.meter.text(value)).transpose()?;
        let (destination, title) = match reference {
            Some((destination, title)) => (
                Some(self.meter.text(destination)?),
                Some(self.meter.text(title)?),
            ),
            None => (None, None),
        };
        self.inline_records
            .try_reserve_exact(1)
            .map_err(|_| MarkdownError::AllocationFailed)?;
        self.inline_records.push(InlineRecord {
            owner,
            span,
            kind,
            text,
            destination,
            title,
        });
        Ok(())
    }

    fn heading(&mut self, level: HeadingLevel, span: RawSpan) -> Result<usize, MarkdownError> {
        let level = match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        };
        self.meter.steps(12)?;
        let mut parent_section = None;
        for (index, heading) in self.headings.iter().enumerate() {
            if index + 1 < usize::from(level) && heading.is_some() {
                parent_section = *heading;
            }
        }
        let ordinal = self.node(
            MarkdownKind::Heading {
                level,
                parent_section,
            },
            span,
            None,
        )?;
        for (index, heading) in self.headings.iter_mut().enumerate() {
            if index + 1 >= usize::from(level) {
                *heading = None;
            }
        }
        self.headings[usize::from(level) - 1] = Some(ordinal);
        self.section = Some(ordinal);
        self.nodes[ordinal].heading_section = Some(ordinal);
        Ok(ordinal)
    }

    fn start(&mut self, tag: Tag<'_>, span: RawSpan) -> Result<(), MarkdownError> {
        add(
            self.stack.len(),
            1,
            self.meter.limits.stack_depth,
            MarkdownError::StackLimit,
        )?;
        let expected = tag.to_end();
        let image = matches!(&tag, Tag::Image { .. });
        let owner = match tag {
            Tag::Paragraph => self.node(MarkdownKind::Paragraph, span, None)?,
            Tag::Heading {
                level,
                id,
                classes,
                attrs,
            } => {
                if id.is_some() || !classes.is_empty() || !attrs.is_empty() {
                    return Err(MarkdownError::UnsupportedEvent);
                }
                self.heading(level, span)?
            }
            Tag::Item => self.node(MarkdownKind::Item, span, None)?,
            Tag::BlockQuote(None) => self.node(MarkdownKind::Quote, span, None)?,
            Tag::HtmlBlock => self.node(MarkdownKind::RawHtmlBlock, span, None)?,
            Tag::CodeBlock(CodeBlockKind::Indented) => {
                self.node(MarkdownKind::IndentedCode, span, None)?
            }
            Tag::CodeBlock(CodeBlockKind::Fenced(info)) => {
                self.node(MarkdownKind::FencedCode, span, Some(&info))?
            }
            Tag::List(_) => self.owner(),
            Tag::Emphasis | Tag::Strong => {
                let kind = if expected == TagEnd::Emphasis {
                    InlineKind::Emphasis
                } else {
                    InlineKind::Strong
                };
                self.inline(kind, span, None, None)?;
                self.owner()
            }
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            }
            | Tag::Image {
                link_type,
                dest_url,
                title,
                ..
            } => {
                let kind = link_kind(link_type)?;
                let kind = if image {
                    InlineKind::Image(kind)
                } else {
                    InlineKind::Link(kind)
                };
                self.inline(kind, span, None, Some((&dest_url, &title)))?;
                self.owner()
            }
            _ => return Err(MarkdownError::UnsupportedEvent),
        };
        self.meter.storage(size_of::<Frame>())?;
        self.meter.steps(1)?;
        self.stack
            .try_reserve_exact(1)
            .map_err(|_| MarkdownError::AllocationFailed)?;
        self.stack.push(Frame {
            expected,
            span,
            owner,
        });
        Ok(())
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>) -> Result<(), MarkdownError> {
        self.meter.used.yielded_events = self
            .meter
            .used
            .yielded_events
            .checked_add(1)
            .ok_or(MarkdownError::ArithmeticOverflow)?;
        self.meter.steps(1)?;
        let span = self.span(range)?;
        match event {
            Event::Start(tag) => self.start(tag, span),
            Event::End(end) => {
                self.meter.steps(1)?;
                let frame = self.stack.last().ok_or(MarkdownError::InvalidStructure)?;
                // 0.13.1 OffsetIter emits the same full item extent on Start/End.
                if frame.expected != end || frame.span != span {
                    return Err(MarkdownError::InvalidStructure);
                }
                self.stack.pop().ok_or(MarkdownError::InvalidStructure)?;
                self.meter.used.owned_live = self
                    .meter
                    .used
                    .owned_live
                    .checked_sub(size_of::<Frame>())
                    .ok_or(MarkdownError::ArithmeticOverflow)?;
                Ok(())
            }
            // Keep only the event observation; even synthesized Text is not a
            // decoded output stream, nor necessarily equal to its source span.
            Event::Text(_) => self.inline(InlineKind::Text, span, None, None),
            Event::Code(text) => self.inline(InlineKind::Code, span, Some(&text), None),
            Event::Html(_) | Event::InlineHtml(_) => {
                self.inline(InlineKind::Html, span, None, None)
            }
            Event::SoftBreak => self.inline(InlineKind::SoftBreak, span, None, None),
            Event::HardBreak => self.inline(InlineKind::HardBreak, span, None, None),
            Event::Rule => self
                .node(MarkdownKind::ThematicBreak, span, None)
                .map(|_| ()),
            _ => Err(MarkdownError::UnsupportedEvent),
        }
    }
}

const fn link_kind(kind: LinkType) -> Result<ObservedLinkKind, MarkdownError> {
    match kind {
        LinkType::Inline => Ok(ObservedLinkKind::Inline),
        LinkType::Reference => Ok(ObservedLinkKind::Reference),
        LinkType::Collapsed => Ok(ObservedLinkKind::Collapsed),
        LinkType::Shortcut => Ok(ObservedLinkKind::Shortcut),
        LinkType::Autolink => Ok(ObservedLinkKind::Autolink),
        LinkType::Email => Ok(ObservedLinkKind::Email),
        // No broken-link callback or wikilink extension is installed.
        _ => Err(MarkdownError::UnsupportedEvent),
    }
}

/// All-or-nothing algorithm seam; original cancellation/deadline are borrowed.
/// Heading sections follow global source preorder, independently of containers.
pub(super) fn parse_markdown<'a>(
    input: &'a [u8],
    limits: MarkdownLimits,
    cancelled: CancellationToken<'_>,
    deadline: Instant,
) -> Result<MarkdownDocument<'a>, MarkdownError> {
    checkpoint(cancelled, deadline)?;
    let result = parse_inner(input, limits, cancelled, deadline);
    // Check the original caller checkpoint before every final success/refusal.
    // A late cancellation also drops any locally completed document.
    checkpoint(cancelled, deadline)?;
    result
}

fn parse_inner<'a>(
    input: &'a [u8],
    limits: MarkdownLimits,
    cancelled: CancellationToken<'_>,
    deadline: Instant,
) -> Result<MarkdownDocument<'a>, MarkdownError> {
    let limits = limits.validate()?.get();
    if input.is_empty() {
        return Err(MarkdownError::EmptyInput);
    }
    if input.len() > limits.input_bytes {
        return Err(MarkdownError::InputLimit);
    }
    if input.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(MarkdownError::BomNotSupported);
    }
    let mut meter = Meter {
        limits,
        used: MarkdownUsage::default(),
    };
    meter.steps(input.len())?;
    let source = std::str::from_utf8(input).map_err(|_| MarkdownError::InvalidUtf8)?;
    meter.storage(size_of::<MarkdownDocument<'_>>())?;
    let mut builder = Builder {
        source,
        nodes: Vec::new(),
        inline_records: Vec::new(),
        stack: Vec::new(),
        headings: [None; 6],
        section: None,
        meter,
    };
    builder.node(
        MarkdownKind::Root,
        RawSpan {
            start: 0,
            end: source.len(),
        },
        None,
    )?;
    checkpoint(cancelled, deadline)?;
    let parser = Parser::new_ext(source, Options::empty());
    checkpoint(cancelled, deadline)?;
    let mut events = parser.into_offset_iter();
    loop {
        checkpoint(cancelled, deadline)?;
        let attempts = add(
            builder.meter.used.next_attempts,
            1,
            builder.meter.limits.next_attempts,
            MarkdownError::AttemptLimit,
        )?;
        builder.meter.steps(1)?;
        builder.meter.used.next_attempts = attempts;
        let next = events.next();
        checkpoint(cancelled, deadline)?;
        let Some((event, range)) = next else {
            break;
        };
        // The attempt was charged even on None. Every Some event, including
        // End/transparent/discarded tags, separately increments yielded usage.
        // The donor may allocate during next; adapter growth is checked below.
        builder.event(event, range)?;
    }
    if !builder.stack.is_empty() {
        return Err(MarkdownError::InvalidStructure);
    }
    drop(events);
    checkpoint(cancelled, deadline)?;
    Ok(MarkdownDocument {
        source,
        nodes: builder.nodes,
        inline_records: builder.inline_records,
        inline_metadata: InlineMetadataStatus::ObservedSpansAndSelectedValues,
        reference_metadata: ReferenceMetadataStatus::PotentiallyIncomplete,
        usage: builder.meter.used,
    })
}
