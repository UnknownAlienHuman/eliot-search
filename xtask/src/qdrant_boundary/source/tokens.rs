//! Streaming tokens over comment/literal-masked Rust, retaining source locations.

#[derive(Clone, Copy, Debug)]
pub(super) struct CodeToken<'a> {
    pub(super) text: &'a str,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) line: usize,
}

pub(super) fn code_tokens(code: &str) -> impl Iterator<Item = &str> {
    code_token_spans(code).map(|token| token.text)
}

pub(super) fn code_token_spans(
    source: &str,
) -> impl Iterator<Item = CodeToken<'_>> {
    let mut code = source;
    let mut line = 1_usize;
    std::iter::from_fn(move || {
        let trimmed = code.trim_start();
        let whitespace = code.len() - trimmed.len();
        line += code[..whitespace].bytes().filter(|byte| *byte == b'\n').count();
        code = trimmed;
        let start = source.len() - code.len();
        let raw_prefix = if code.strip_prefix("r#").is_some_and(|raw| {
            raw.chars().next().is_some_and(is_identifier_character)
        }) {
            2
        } else {
            0
        };
        let tail = &code[raw_prefix..];
        let first = tail.chars().next()?;
        let end = if is_identifier_character(first) {
            tail.find(|character| !is_identifier_character(character))
                .unwrap_or(tail.len())
        } else {
            first.len_utf8()
        };
        let (text, rest) = code.split_at(raw_prefix + end);
        code = rest;
        Some(CodeToken {
            text,
            start,
            end: start + text.len(),
            line,
        })
    })
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '_'
        || (!character.is_ascii() && !character.is_whitespace())
}
