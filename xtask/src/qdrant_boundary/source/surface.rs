use std::collections::BTreeSet;
use std::iter::Peekable;

use super::imports::contains_any_identifier;
use super::tokens::{CodeToken, code_token_spans};

pub(super) fn find_public_vendor_surfaces(
    code: &str,
    vendor_identifiers: &BTreeSet<String>,
) -> Vec<usize> {
    find_public_surfaces_matching(code, |surface| {
        contains_any_identifier(surface, vendor_identifiers)
    })
}

/// Inspects lexical public surfaces independently of source formatting.
///
/// Borrowed slices preserve the original line/byte locations. Function bodies,
/// constant initializers and private named fields are not public signatures.
/// Trait/enum bodies, tuple-struct signatures and exported macros are checked
/// as a whole. This does not expand macros or replace Rust name/type checking.
/// Unterminated surfaces are inspected through EOF rather than silently dropped.
pub(super) fn find_public_surfaces_matching(
    code: &str,
    mut matches: impl FnMut(&str) -> bool,
) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut tokens = code_token_spans(code).peekable();
    let mut macro_export_line = None;

    while let Some(token) = tokens.next() {
        if token.text == "#" && tokens.peek().is_some_and(|next| next.text == "[") {
            let _ = tokens.next();
            if tokens.peek().is_some_and(|next| next.text == "macro_export") {
                macro_export_line = Some(token.line);
            }
            skip_group(&mut tokens, code.len());
            continue;
        }
        if token.text == "macro_rules"
            && tokens.peek().is_some_and(|next| next.text == "!")
        {
            let end = macro_body_end(&mut tokens, code.len());
            if let Some(line) = macro_export_line.take()
                && matches(&code[token.start..end])
            {
                violations.push(line);
            }
            // An unexported macro definition is not an expanded public API.
            continue;
        }
        macro_export_line = None;
        if token.text != "pub"
            || restricted_visibility(&code[token.end..])
        {
            continue;
        }
        let end = public_surface_end(&mut tokens, code.len());
        if matches(&code[token.start..end]) {
            violations.push(token.line);
        }
    }

    violations.sort_unstable();
    violations.dedup();
    violations
}

fn restricted_visibility(code: &str) -> bool {
    let mut tokens = code_token_spans(code);
    if !tokens.next().is_some_and(|token| token.text == "(") {
        return false;
    }
    match tokens.next().map(|token| token.text) {
        Some("in") => true,
        Some("crate" | "self" | "super") => {
            tokens.next().is_some_and(|token| token.text == ")")
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceKind {
    Function,
    Constant,
    Statement,
    Struct,
    Module,
    Block,
    Field,
}

fn surface_kind<'a>(
    tokens: &mut Peekable<impl Iterator<Item = CodeToken<'a>>>,
) -> SurfaceKind {
    while let Some(token) = tokens.next() {
        match token.text {
            "async" | "unsafe" | "extern" | "auto" | "default" | "safe" => {}
            "const" if tokens.peek().is_some_and(|next| {
                matches!(next.text, "fn" | "unsafe")
            }) => {}
            "fn" => {
                return if tokens.peek().is_some_and(|next| next.text == "(") {
                    SurfaceKind::Field
                } else {
                    SurfaceKind::Function
                };
            }
            "const" | "static" => return SurfaceKind::Constant,
            "type" | "use" => return SurfaceKind::Statement,
            "struct" | "union" => return SurfaceKind::Struct,
            "mod" => return SurfaceKind::Module,
            "enum" | "trait" | "macro" => return SurfaceKind::Block,
            _ => return SurfaceKind::Field,
        }
    }
    SurfaceKind::Field
}

fn public_surface_end<'a>(
    tokens: &mut Peekable<impl Iterator<Item = CodeToken<'a>>>,
    eof: usize,
) -> usize {
    // Classify without losing the first field token: tuple fields can start
    // with a delimiter, as in `pub (u8, Vendor)`, not only a field name.
    let kind = if tokens.peek().is_some_and(|token| {
        matches!(token.text, "(" | "[" | "<" | "&" | "*")
    }) {
        SurfaceKind::Field
    } else {
        surface_kind(tokens)
    };
    let mut depth = SurfaceDepth::default();
    let mut previous = "";

    while let Some(token) = tokens.next() {
        if depth.at_top() {
            match token.text {
                ";" => return token.end,
                "," | ")" | "}" if kind == SurfaceKind::Field => return token.start,
                "=" if kind == SurfaceKind::Constant => return token.start,
                "{" if kind == SurfaceKind::Block => return skip_group(tokens, eof),
                "{" if matches!(kind, SurfaceKind::Function | SurfaceKind::Struct | SurfaceKind::Module) => {
                    return token.start;
                }
                _ => {}
            }
        }
        depth.observe(token.text, previous);
        previous = token.text;
    }
    eof
}

// Only ordinary delimiter groups require balancing inside a generic argument.
// A `>` in `fn() -> T` is an arrow, not the end of that generic argument. Angle
// punctuation inside array/const expressions must not alter the outer type depth.
#[derive(Clone, Copy, Debug, Default)]
struct SurfaceDepth {
    round: usize,
    square: usize,
    curly: usize,
    angle: usize,
}

impl SurfaceDepth {
    const fn groups_closed(self) -> bool {
        self.round == 0 && self.square == 0 && self.curly == 0
    }

    const fn at_top(self) -> bool {
        self.groups_closed() && self.angle == 0
    }

    fn observe(&mut self, token: &str, previous: &str) {
        match token {
            "(" => self.round = self.round.saturating_add(1),
            ")" => self.round = self.round.saturating_sub(1),
            "[" => self.square = self.square.saturating_add(1),
            "]" => self.square = self.square.saturating_sub(1),
            "{" => self.curly = self.curly.saturating_add(1),
            "}" => self.curly = self.curly.saturating_sub(1),
            "<" if self.groups_closed() => self.angle = self.angle.saturating_add(1),
            ">" if self.groups_closed() && previous != "-" => {
                self.angle = self.angle.saturating_sub(1);
            }
            _ => {}
        }
    }
}

fn macro_body_end<'a>(
    tokens: &mut impl Iterator<Item = CodeToken<'a>>,
    eof: usize,
) -> usize {
    while let Some(token) = tokens.next() {
        if matches!(token.text, "{" | "(" | "[") {
            return skip_group(tokens, eof);
        }
    }
    eof
}

// The opening delimiter has already been consumed. Counter-only traversal is
// iterative and bounded by the existing file-byte budget; no recursive descent,
// token buffer or additional source copy is created, even for deeply nested input.
fn skip_group<'a>(
    tokens: &mut impl Iterator<Item = CodeToken<'a>>,
    eof: usize,
) -> usize {
    let mut depth = 1_usize;
    for token in tokens {
        match token.text {
            "{" | "(" | "[" => depth = depth.saturating_add(1),
            "}" | ")" | "]" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return token.end;
                }
            }
            _ => {}
        }
    }
    eof
}
