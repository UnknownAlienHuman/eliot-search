use std::collections::BTreeSet;

use super::imports::contains_any_identifier;

pub(super) fn find_public_vendor_surfaces(
    code: &str,
    vendor_identifiers: &BTreeSet<String>,
) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut public_signature: Option<(usize, String)> = None;
    let mut public_block: Option<PublicBlock> = None;
    let mut macro_export_attribute: Option<usize> = None;
    let mut public_macro: Option<PublicBlock> = None;

    for (index, line) in code.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if let Some(item) = macro_export_suffix(trimmed) {
            macro_export_attribute = Some(line_number);
            if starts_macro_rules(item) {
                public_macro = Some(PublicBlock::new(line_number));
                macro_export_attribute = None;
            }
        } else if let Some(attribute_line) = macro_export_attribute {
            if starts_macro_rules(trimmed) {
                public_macro = Some(PublicBlock::new(attribute_line));
                macro_export_attribute = None;
            } else if !trimmed.is_empty() && !trimmed.starts_with("#[") {
                macro_export_attribute = None;
            }
        }

        if public_macro.is_none() && starts_public_macro(trimmed) {
            public_macro = Some(PublicBlock::new(line_number));
        }
        if let Some(block) = public_macro.as_mut() {
            if contains_any_identifier(trimmed, vendor_identifiers) {
                violations.push(block.start);
            }
            block.observe_tokens(trimmed);
            if block.finished() {
                public_macro = None;
            }
        }

        if public_block.is_none() && starts_public_block(trimmed) {
            public_block = Some(PublicBlock::new(line_number));
        }
        if let Some(block) = public_block.as_mut() {
            if contains_any_identifier(trimmed, vendor_identifiers) {
                violations.push(block.start);
            }
            block.observe_braces(trimmed);
            if block.finished() {
                public_block = None;
            }
        }

        if public_signature.is_none() && starts_public_signature(trimmed) {
            public_signature = Some((line_number, String::new()));
        }

        let mut clear_signature = false;
        if let Some((start, signature)) = public_signature.as_mut() {
            signature.push_str(trimmed);
            signature.push('\n');
            if contains_any_identifier(signature, vendor_identifiers) {
                violations.push(*start);
                clear_signature = true;
            } else if signature_ended(trimmed) {
                clear_signature = true;
            }
        }
        if clear_signature {
            public_signature = None;
        }

        if trimmed.starts_with("pub ")
            && contains_any_identifier(trimmed, vendor_identifiers)
        {
            violations.push(line_number);
        }
    }

    violations.sort_unstable();
    violations.dedup();
    violations
}

#[derive(Clone, Copy, Debug)]
struct PublicBlock {
    start: usize,
    depth: isize,
    opened: bool,
}

impl PublicBlock {
    const fn new(start: usize) -> Self {
        Self {
            start,
            depth: 0,
            opened: false,
        }
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

    const fn finished(self) -> bool {
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
