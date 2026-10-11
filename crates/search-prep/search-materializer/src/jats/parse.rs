//! Borrowed quick-xml pull events; no dynamic tree, resolver, DTD or network.
use super::meter::{Meter, debit};
use super::model::{
    Inventory, JatsAttribute, JatsAttributeKind as A, JatsComment as Comment, JatsElement,
    JatsElementKind as K, JatsError as E, JatsSpan, JatsText, JatsTextKind,
};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesDecl, BytesStart, Event};
use quick_xml::name::QName;

const XLINK: &str = "http://www.w3.org/1999/xlink";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

pub(super) const fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}'
        | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
fn whitespace(value: &str) -> bool {
    value
        .bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}
fn raw(source: &str, span: JatsSpan, cap: usize) -> Result<&str, E> {
    let value = source.get(span.start..span.end).ok_or(E::InvalidRange)?;
    if value.len() > cap {
        return Err(E::LimitExceeded);
    }
    Ok(value)
}
fn delimited(raw: &str, left: &str, right: &str, body: &str) -> Result<(), E> {
    if raw.strip_prefix(left).and_then(|v| v.strip_suffix(right)) != Some(body) {
        return Err(E::InvalidRange);
    }
    Ok(())
}
fn parent(kind: K, previous: Option<K>) -> Result<(), E> {
    let valid = match kind {
        K::Article => previous.is_none(),
        K::Front | K::Body | K::Back => previous == Some(K::Article),
        K::ArticleMeta | K::JournalMeta => previous == Some(K::Front),
        K::JournalId | K::Issn => previous == Some(K::JournalMeta),
        K::TitleGroup => previous == Some(K::ArticleMeta),
        K::ArticleTitle => matches!(
            previous,
            Some(K::TitleGroup | K::MixedCitation | K::ElementCitation)
        ),
        K::Subtitle => previous == Some(K::TitleGroup),
        K::MixedCitation | K::ElementCitation => previous == Some(K::Reference),
        K::Table => previous == Some(K::TableWrap),
        K::TableHead | K::TableBody => previous == Some(K::Table),
        K::Row => matches!(previous, Some(K::Table | K::TableHead | K::TableBody)),
        K::HeaderCell | K::Cell => previous == Some(K::Row),
        K::Reference => previous == Some(K::ReferenceList),
        K::Footnote => previous == Some(K::FootnoteGroup),
        _ => previous.is_some(),
    };
    if valid {
        Ok(())
    } else {
        Err(E::UnsupportedSyntax)
    }
}
fn attribute_parent(kind: A, element: K) -> Result<(), E> {
    let valid = match kind {
        A::DtdVersion | A::ArticleType | A::XlinkNamespace | A::XmlNamespace => {
            element == K::Article
        }
        A::JournalIdType => element == K::JournalId,
        A::PublicationFormat => element == K::Issn,
        A::Rid | A::RefType => element == K::CrossReference,
        A::Href | A::XlinkType => element == K::ExternalLink,
        A::SectionType => element == K::Section,
        A::ListType => element == K::List,
        A::PublicationType => matches!(element, K::MixedCitation | K::ElementCitation),
        A::Colspan | A::Rowspan | A::Scope | A::Headers => {
            matches!(element, K::HeaderCell | K::Cell)
        }
        A::Align | A::Valign => matches!(
            element,
            K::Table | K::TableHead | K::TableBody | K::Row | K::HeaderCell | K::Cell
        ),
        A::Position | A::Orientation => matches!(element, K::Figure | K::TableWrap),
        A::Id | A::Lang | A::Space | A::SpecificUse | A::ContentType => true,
    };
    if valid {
        Ok(())
    } else {
        Err(E::UnsupportedSyntax)
    }
}
fn attribute_separators(start: &BytesStart<'_>, meter: &mut Meter<'_>) -> Result<(), E> {
    // The donor validates quoting/duplicates, but permits adjacent attributes.
    // Check only the missing XML whitespace rule on its borrowed raw slice.
    let bytes = start.attributes_raw().as_bytes();
    meter.work(bytes.len())?;
    let space = |b| matches!(b, b' ' | b'\t' | b'\n' | b'\r');
    if bytes.first().is_some_and(|&b| !space(b)) {
        return Err(E::MalformedXml);
    }
    let mut quote = None;
    for (index, &byte) in bytes.iter().enumerate() {
        meter.poll()?;
        if quote == Some(byte) {
            quote = None;
            if bytes.get(index + 1).is_some_and(|&b| !space(b)) {
                return Err(E::MalformedXml);
            }
        } else if quote.is_none() && matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        }
    }
    meter.poll()
}
fn declaration(event: &BytesDecl<'_>, meter: &mut Meter<'_>) -> Result<(), E> {
    if event.version().map_err(|_| E::MalformedXml)?.as_ref() != "1.0" {
        return Err(E::UnsupportedSyntax);
    }
    let start = BytesStart::from_content(event.as_ref(), 3);
    attribute_separators(&start, meter)?;
    let mut count = 0;
    let mut order = 0;
    let mut attrs = start.attributes();
    attrs.with_checks(true);
    loop {
        meter.poll()?;
        meter.work(1)?;
        let next = attrs.next();
        meter.poll()?;
        let Some(attr) = next else { break };
        count = debit(count, 1, 3)?;
        let attr = attr.map_err(|_| E::MalformedXml)?;
        meter.work(attr.value.len())?;
        if attr.value.len() > meter.parser().attribute_bytes {
            return Err(E::LimitExceeded);
        }
        let field = match attr.key.as_ref() {
            "version" => 1,
            "encoding" => 2,
            "standalone" => 3,
            _ => return Err(E::UnsupportedSyntax),
        };
        if field <= order || (count == 1 && field != 1) {
            return Err(E::MalformedXml);
        }
        order = field;
        let valid = match attr.key.as_ref() {
            "version" => attr.value.as_ref() == "1.0",
            "encoding" => attr.value.eq_ignore_ascii_case("UTF-8"),
            "standalone" => matches!(attr.value.as_ref(), "yes" | "no"),
            _ => false,
        };
        if !valid {
            return Err(E::UnsupportedSyntax);
        }
    }
    meter.poll()
}
fn attributes(
    start: &BytesStart<'_>,
    owner: usize,
    span: JatsSpan,
    out: &mut Inventory,
    xlink: &mut bool,
    meter: &mut Meter<'_>,
) -> Result<(), E> {
    let limits = meter.parser();
    attribute_separators(start, meter)?;
    let mut borrowed = Vec::new();
    let mut attrs = start.attributes();
    attrs.with_checks(true);
    loop {
        meter.poll()?;
        meter.work(1)?;
        let next = attrs.next();
        meter.poll()?;
        let Some(value) = next else { break };
        if borrowed.len() >= limits.attributes_each
            || out
                .attributes
                .len()
                .checked_add(borrowed.len())
                .ok_or(E::Overflow)?
                >= limits.attributes
        {
            return Err(E::LimitExceeded);
        }
        let attr = value.map_err(|_| E::MalformedXml)?;
        let kind = A::from_name(attr.key.as_ref())?;
        attribute_parent(kind, out.elements[owner].kind)?;
        if attr.value.len() > limits.attribute_bytes {
            return Err(E::LimitExceeded);
        }
        meter.work(attr.value.len())?;
        if attr.value.contains('<') {
            return Err(E::MalformedXml);
        }
        match kind {
            A::XlinkNamespace | A::XmlNamespace => {
                if owner != 0 {
                    return Err(E::UnsupportedSyntax);
                }
                out.namespaces = debit(out.namespaces, 1, limits.namespaces)?;
                let expected = if kind == A::XlinkNamespace {
                    XLINK
                } else {
                    XML
                };
                if attr.value.as_ref() != expected {
                    return Err(E::UnsupportedSyntax);
                }
                if kind == A::XlinkNamespace {
                    *xlink = true;
                }
            }
            A::DtdVersion if attr.value.as_ref() != "1.4" => return Err(E::UnsupportedSyntax),
            A::Space if !matches!(attr.value.as_ref(), "default" | "preserve") => {
                return Err(E::UnsupportedSyntax);
            }
            _ => {}
        }
        meter.push(&mut borrowed, (kind, attr), limits.attributes_each)?;
    }
    for (kind, attr) in borrowed {
        meter.poll()?;
        if matches!(kind, A::Href | A::XlinkType) && !*xlink {
            return Err(E::UnsupportedSyntax);
        }
        // Depth1 predefined/numeric normalization cannot grow beyond raw value.
        // Reserve the donor temporary before calling it; cumulative, no refunds.
        meter.own(attr.value.len())?;
        meter.work(attr.value.len())?;
        let decoded = attr
            .normalized_value_with(
                XmlVersion::Explicit1_0,
                1,
                quick_xml::escape::resolve_xml_entity,
            )
            .map_err(|_| E::UnsupportedSyntax)?;
        meter.poll()?;
        if decoded.len() > attr.value.len() || !decoded.chars().all(xml_char) {
            return Err(E::InvalidCharacter);
        }
        if decoded.len() > limits.mapped_bytes_each {
            return Err(E::LimitExceeded);
        }
        out.mapped = debit(out.mapped, decoded.len(), limits.mapped_bytes)?;
        let value = meter.copy(&decoded)?;
        meter.push(
            &mut out.attributes,
            JatsAttribute {
                owner,
                kind,
                span,
                value,
            },
            limits.attributes,
        )?;
    }
    meter.poll()
}
fn text(
    out: &mut Inventory,
    stack: &[Frame],
    span: JatsSpan,
    kind: JatsTextKind,
    value: &str,
    meter: &mut Meter<'_>,
) -> Result<(), E> {
    let owner = stack.last().ok_or(E::UnsupportedSyntax)?.ordinal;
    let limits = meter.parser();
    if out.texts.len() >= limits.texts || value.len() > limits.mapped_bytes_each {
        return Err(E::LimitExceeded);
    }
    if out.elements[owner].kind == K::ElementCitation && !whitespace(value) {
        return Err(E::UnsupportedSyntax);
    }
    meter.work(value.len())?;
    if !value.chars().all(xml_char) {
        return Err(E::InvalidCharacter);
    }
    out.mapped = debit(out.mapped, value.len(), limits.mapped_bytes)?;
    let value = meter.copy(value)?;
    let item = JatsText {
        owner,
        event: out.events - 1,
        span,
        kind,
        value,
    };
    meter.push(&mut out.texts, item, limits.texts)
}
#[derive(Default)]
struct StructureCounts {
    sections: usize,
    figures: usize,
    tables: usize,
    references: usize,
}
fn text_slot(out: &Inventory, stack: &[Frame], meter: &Meter<'_>) -> Result<(), E> {
    if stack.is_empty() {
        return Err(E::UnsupportedSyntax);
    }
    if out.texts.len() >= meter.parser().texts {
        return Err(E::LimitExceeded);
    }
    meter.poll()
}
impl StructureCounts {
    fn add(&mut self, kind: K, meter: &Meter<'_>) -> Result<(), E> {
        let limits = meter.parser();
        let target = match kind {
            K::Section => Some((&mut self.sections, limits.sections)),
            K::Figure => Some((&mut self.figures, limits.figures)),
            K::Table => Some((&mut self.tables, limits.tables)),
            K::Reference => Some((&mut self.references, limits.references)),
            _ => None,
        };
        if let Some((count, ceiling)) = target {
            *count = debit(*count, 1, ceiling)?;
        }
        Ok(())
    }
}
struct Frame {
    ordinal: usize,
    stage: u8,
    journal_ids: usize,
    issns: usize,
}
impl Frame {
    const fn new(ordinal: usize) -> Self {
        Self {
            ordinal,
            stage: 0,
            journal_ids: 0,
            issns: 0,
        }
    }
    const fn child(&mut self, parent: K, child: K) -> Result<(), E> {
        match parent {
            K::Article => {
                self.stage = match (self.stage, child) {
                    (0, K::Front) => 1,
                    (1, K::Body) => 2,
                    (1 | 2, K::Back) => 3,
                    _ => return Err(E::UnsupportedSyntax),
                };
            }
            K::Front => {
                self.stage = match (self.stage, child) {
                    (0, K::JournalMeta) => 1,
                    (1, K::ArticleMeta) => 2,
                    _ => return Err(E::UnsupportedSyntax),
                };
            }
            K::JournalMeta => match child {
                K::JournalId if self.issns == 0 => self.journal_ids += 1,
                K::Issn if self.journal_ids > 0 => self.issns += 1,
                _ => return Err(E::UnsupportedSyntax),
            },
            K::TitleGroup => match child {
                K::ArticleTitle if self.stage == 0 => self.stage = 1,
                K::Subtitle if self.stage == 1 => {}
                _ => return Err(E::UnsupportedSyntax),
            },
            _ => {}
        }
        Ok(())
    }
    const fn finish(&self, kind: K) -> Result<(), E> {
        let valid = match kind {
            K::Article => self.stage > 0,
            K::Front => self.stage == 2,
            K::JournalMeta => self.journal_ids > 0 && self.issns > 0,
            K::TitleGroup => self.stage == 1,
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(E::UnsupportedSyntax)
        }
    }
}
fn open(
    start: &BytesStart<'_>,
    span: JatsSpan,
    empty: bool,
    stack: &mut Vec<Frame>,
    out: &mut Inventory,
    state: &mut (bool, StructureCounts),
    meter: &mut Meter<'_>,
) -> Result<(), E> {
    let limits = meter.parser();
    if stack.len() >= limits.depth || out.elements.len() >= limits.elements {
        return Err(E::LimitExceeded);
    }
    let kind = K::from_name(start.name().as_ref())?;
    let owner = stack.last().map(|frame| frame.ordinal);
    parent(kind, owner.map(|i| out.elements[i].kind))?;
    if owner.is_none() && !out.elements.is_empty() {
        return Err(E::MalformedXml);
    }
    if let Some(frame) = stack.last_mut() {
        frame.child(out.elements[frame.ordinal].kind, kind)?;
    }
    state.1.add(kind, meter)?;
    let ordinal = out.elements.len();
    let item = JatsElement {
        kind,
        parent: owner,
        event: out.events - 1,
        close_event: out.events - 1,
        opening: span,
        closing: None,
        span,
    };
    meter.push(&mut out.elements, item, limits.elements)?;
    attributes(start, ordinal, span, out, &mut state.0, meter)?;
    let frame = Frame::new(ordinal);
    if empty {
        frame.finish(kind)?;
    } else {
        meter.push(stack, frame, limits.depth)?;
    }
    meter.poll()
}

pub(super) fn parse(source: &str, meter: &mut Meter<'_>) -> Result<Inventory, E> {
    let limits = meter.parser();
    if source.is_empty() || source.len() > limits.input_bytes {
        return Err(E::LimitExceeded);
    }
    meter.work(source.len())?;
    if source.starts_with('\u{feff}') {
        return Err(E::UnsupportedSyntax);
    }
    if !source.chars().all(xml_char) {
        return Err(E::InvalidCharacter);
    }
    let mut reader = Reader::from_str(source);
    let config = reader.config_mut();
    config.allow_dangling_amp = false;
    config.allow_unmatched_ends = false;
    config.check_comments = true;
    config.check_end_names = true;
    config.expand_empty_elements = false;
    config.trim_text_start = false;
    config.trim_text_end = false;
    config.trim_markup_names_in_closing_tags = true;
    let mut out = Inventory::default();
    let mut stack = Vec::new();
    let mut state = (false, StructureCounts::default());
    loop {
        meter.poll()?;
        out.events = debit(out.events, 1, limits.events)?; // includes terminal EOF
        meter.work(1)?;
        let before = usize::try_from(reader.buffer_position()).map_err(|_| E::Overflow)?;
        let result = reader.read_event();
        meter.poll()?;
        let event = result.map_err(|_| E::MalformedXml)?;
        let after = usize::try_from(reader.buffer_position()).map_err(|_| E::Overflow)?;
        let span = JatsSpan {
            start: before,
            end: after,
        };
        let original = raw(source, span, limits.event_bytes)?;
        meter.work(original.len())?;
        match event {
            Event::Start(start) => {
                delimited(original, "<", ">", start.as_ref())?;
                open(&start, span, false, &mut stack, &mut out, &mut state, meter)?;
            }
            Event::Empty(start) => {
                delimited(original, "<", "/>", start.as_ref())?;
                open(&start, span, true, &mut stack, &mut out, &mut state, meter)?;
            }
            Event::End(end) => {
                let body = original
                    .strip_prefix("</")
                    .and_then(|s| s.strip_suffix('>'))
                    .ok_or(E::InvalidRange)?;
                if body.trim_end_matches([' ', '\t', '\r', '\n']) != end.name().as_ref() {
                    return Err(E::InvalidRange);
                }
                let frame = stack.pop().ok_or(E::MalformedXml)?;
                let index = frame.ordinal;
                frame.finish(out.elements[index].kind)?;
                if QName(out.elements[index].kind.name()) != end.name() {
                    return Err(E::MalformedXml);
                }
                out.elements[index].closing = Some(span);
                out.elements[index].close_event = out.events - 1;
                out.elements[index].span.end = after;
            }
            Event::Text(value) => {
                if original != value.as_ref() || original.contains("]]>") {
                    return Err(E::MalformedXml);
                }
                if stack.is_empty() {
                    if !whitespace(original) {
                        return Err(E::MalformedXml);
                    }
                } else {
                    text_slot(&out, &stack, meter)?;
                    meter.own(original.len())?;
                    meter.work(original.len())?;
                    let normalized = value.xml10_content();
                    meter.poll()?;
                    text(
                        &mut out,
                        &stack,
                        span,
                        JatsTextKind::Text,
                        &normalized,
                        meter,
                    )?;
                }
            }
            Event::CData(value) => {
                delimited(original, "<![CDATA[", "]]>", value.as_ref())?;
                text_slot(&out, &stack, meter)?;
                meter.own(value.len())?;
                meter.work(value.len())?;
                let normalized = value.xml10_content();
                meter.poll()?;
                text(
                    &mut out,
                    &stack,
                    span,
                    JatsTextKind::Cdata,
                    &normalized,
                    meter,
                )?;
            }
            Event::GeneralRef(value) => {
                delimited(original, "&", ";", value.as_ref())?;
                text_slot(&out, &stack, meter)?;
                meter.work(value.len())?;
                let resolved = value.resolve_char_ref().map_err(|_| E::UnsupportedSyntax)?;
                meter.poll()?;
                let c = match resolved {
                    Some(c) if xml_char(c) => c,
                    Some(_) => return Err(E::InvalidCharacter),
                    None => match value.as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "apos" => '\'',
                        "quot" => '"',
                        _ => return Err(E::UnsupportedSyntax),
                    },
                };
                let mut buffer = [0_u8; 4];
                text(
                    &mut out,
                    &stack,
                    span,
                    JatsTextKind::Reference,
                    c.encode_utf8(&mut buffer),
                    meter,
                )?;
            }
            Event::Comment(value) => {
                delimited(original, "<!--", "-->", value.as_ref())?;
                let comment = Comment {
                    owner: stack.last().map(|frame| frame.ordinal),
                    event: out.events - 1,
                    span,
                };
                meter.push(&mut out.comments, comment, limits.comments)?;
            }
            Event::Decl(value) => {
                if before != 0 || out.events != 1 || out.declaration.is_some() {
                    return Err(E::UnsupportedSyntax);
                }
                delimited(original, "<?", "?>", value.as_ref())?;
                declaration(&value, meter)?;
                out.declaration = Some(span);
            }
            Event::Eof => {
                if before != source.len()
                    || after != before
                    || !stack.is_empty()
                    || out.elements.is_empty()
                {
                    return Err(E::MalformedXml);
                }
                meter.poll()?;
                return Ok(out);
            }
            Event::DocType(_) | Event::PI(_) => return Err(E::UnsupportedSyntax),
        }
        meter.poll()?;
    }
}
