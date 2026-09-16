use std::collections::BTreeSet;

use super::imports::contains_any_identifier;

pub(super) fn find_public_vendor_surfaces(
    code: &str,
    vendor_identifiers: &BTreeSet<String>,
) -> Vec<usize> {
    find_public_surfaces_matching(code, |surface| {
        contains_any_identifier(surface, vendor_identifiers)
    })
}

/// Returns the start lines of public Rust surfaces for which `matches` is true.
///
/// Signatures, trait/enum bodies and exported macro token trees are accumulated
/// before matching so a qualified path split across lines cannot evade the
/// lexical boundary check. Unterminated public surfaces are still inspected at
/// EOF; compilation remains the complementary syntax authority.
pub(super) fn find_public_surfaces_matching(
    code: &str,
    mut matches: impl FnMut(&str) -> bool,
) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut public_signature: Option<PublicSurface> = None;
    let mut public_block: Option<PublicSurface> = None;
    let mut macro_export_attribute: Option<usize> = None;
    let mut public_macro: Option<PublicSurface> = None;

    for (index, line) in code.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if let Some(item) = macro_export_suffix(trimmed) {
            macro_export_attribute = Some(line_number);
            if starts_macro_rules(item) {
                public_macro = Some(PublicSurface::new(line_number));
                macro_export_attribute = None;
            }
        } else if let Some(attribute_line) = macro_export_attribute {
            if starts_macro_rules(trimmed) {
                public_macro = Some(PublicSurface::new(attribute_line));
                macro_export_attribute = None;
            } else if !trimmed.is_empty() && !trimmed.starts_with("#[") {
                macro_export_attribute = None;
            }
        }

        if public_macro.is_none() && starts_public_macro(trimmed) {
            public_macro = Some(PublicSurface::new(line_number));
        }
        if let Some(surface) = public_macro.as_mut() {
            surface.push_line(trimmed);
            surface.observe_tokens(trimmed);
            if surface.finished() {
                if matches(&surface.text) {
                    violations.push(surface.start);
                }
                public_macro = None;
            }
        }

        if public_block.is_none() && starts_public_block(trimmed) {
            public_block = Some(PublicSurface::new(line_number));
        }
        if let Some(surface) = public_block.as_mut() {
            surface.push_line(trimmed);
            surface.observe_braces(trimmed);
            if surface.finished() {
                if matches(&surface.text) {
                    violations.push(surface.start);
                }
                public_block = None;
            }
        }

        if public_signature.is_none() && starts_public_signature(trimmed) {
            public_signature = Some(PublicSurface::new(line_number));
        }
        if let Some(surface) = public_signature.as_mut() {
            surface.push_line(trimmed);
            if signature_ended(trimmed) {
                if matches(&surface.text) {
                    violations.push(surface.start);
                }
                public_signature = None;
            }
        }

        // Public fields and compact declarations can sit inside a public
        // struct body even though the struct header itself ended at `{`.
        if trimmed.starts_with("pub ") && matches(trimmed) {
            violations.push(line_number);
        }
    }

    for surface in [public_signature, public_block, public_macro]
        .into_iter()
        .flatten()
    {
        if matches(&surface.text) {
            violations.push(surface.start);
        }
    }

    violations.sort_unstable();
    violations.dedup();
    violations
}

#[derive(Clone, Debug)]
struct PublicSurface {
    start: usize,
    depth: isize,
    opened: bool,
    text: String,
}

impl PublicSurface {
    const fn new(start: usize) -> Self {
        Self {
            start,
            depth: 0,
            opened: false,
            text: String::new(),
        }
    }

    fn push_line(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
    }

    fn observe_braces(&mut self, line: &str) {
        self.observe_counts(
            line.bytes().filter(|byte| *byte == b'{').count(),
            line.bytes().filter(|byte| *byte == b'}').count(),
        );
    }

    fn observe_tokens(&mut self, line: &str) {
        self.observe_counts(
            line.bytes()
                .filter(|byte| matches!(*byte, b'{' | b'(' | b'['))
                .count(),
            line.bytes()
                .filter(|byte| matches!(*byte, b'}' | b')' | b']'))
                .count(),
        );
    }

    fn observe_counts(&mut self, opens: usize, closes: usize) {
        if opens > 0 {
            self.opened = true;
        }
        self.depth += isize::try_from(opens).unwrap_or(isize::MAX);
        self.depth -= isize::try_from(closes).unwrap_or(isize::MAX);
    }

    const fn finished(&self) -> bool {
        self.opened && self.depth <= 0
    }
}

fn macro_export_suffix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("#[macro_export")?;
    if !rest.starts_with(']') && !rest.starts_with('(') {
        return None;
    }
    let end = rest.find(']')?;
    Some(rest[end + 1..].trim_start())
}

fn starts_macro_rules(line: &str) -> bool {
    let compact = line.replace(char::is_whitespace, "");
    compact.starts_with("macro_rules!")
}

fn starts_public_macro(line: &str) -> bool {
    line.strip_prefix("pub ")
        .is_some_and(|rest| rest.starts_with("macro "))
}

fn starts_public_block(line: &str) -> bool {
    line.starts_with("pub trait ")
        || line.starts_with("pub unsafe trait ")
        || line.starts_with("pub auto trait ")
        || line.starts_with("pub enum ")
}

fn starts_public_signature(line: &str) -> bool {
    if line == "pub" {
        return true;
    }
    let Some(rest) = line.strip_prefix("pub ") else {
        return false;
    };
    matches!(
        rest.split_whitespace().next(),
        Some(
            "fn" | "async" | "unsafe" | "const" | "extern" | "type"
                | "static" | "struct" | "union" | "use"
        )
    )
}

fn signature_ended(line: &str) -> bool {
    line.contains('{') || line.ends_with(';')
}
