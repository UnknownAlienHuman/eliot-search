use std::collections::BTreeSet;

use super::imports::contains_any_identifier;

pub(super) fn find_public_vendor_surfaces(
    code: &str,
    vendor_identifiers: &BTreeSet<String>,
) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut public_signature: Option<(usize, String)> = None;
    let mut public_block: Option<PublicBlock> = None;

    for (index, line) in code.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if public_block.is_none() && starts_public_block(trimmed) {
            public_block = Some(PublicBlock::new(line_number));
        }
        if let Some(block) = public_block.as_mut() {
            if contains_any_identifier(trimmed, vendor_identifiers) {
                violations.push(block.start);
            }
            block.observe(trimmed);
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

    fn observe(&mut self, line: &str) {
        let opens = line.bytes().filter(|byte| *byte == b'{').count();
        let closes = line.bytes().filter(|byte| *byte == b'}').count();
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

fn starts_public_block(line: &str) -> bool {
    line.starts_with("pub trait ")
        || line.starts_with("pub unsafe trait ")
        || line.starts_with("pub auto trait ")
        || line.starts_with("pub enum ")
}

fn starts_public_signature(line: &str) -> bool {
    line.starts_with("pub fn ")
        || line.starts_with("pub async fn ")
        || line.starts_with("pub unsafe fn ")
        || line.starts_with("pub const fn ")
        || line.starts_with("pub extern ")
        || line.starts_with("pub type ")
        || line.starts_with("pub static ")
        || line.starts_with("pub const ")
        || line.starts_with("pub struct ")
        || line.starts_with("pub union ")
        || line.starts_with("pub use ")
        || line.starts_with("pub extern crate ")
}

fn signature_ended(line: &str) -> bool {
    line.contains('{') || line.ends_with(';')
}
