//! Bounded, token-delimited declarations over already masked Rust source.

use std::iter::Peekable;

use super::tokens::{CodeToken, code_token_spans, code_tokens};

const MAX_STATEMENTS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StatementKind {
    Use,
    ExternCrate,
    TypeAlias,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Statement<'a> {
    pub(super) kind: StatementKind,
    pub(super) line: usize,
    pub(super) public: bool,
    pub(super) body: &'a str,
    pub(super) complete: bool,
}

/// Returns borrowed declaration bodies, never whole source lines. An exhausted
/// declaration budget is an error at the first omitted declaration, not a prefix.
pub(super) fn collect_statements(code: &str) -> Result<Vec<Statement<'_>>, usize> {
    let mut statements = Vec::new();
    let mut tokens = code_token_spans(code).peekable();
    while let Some(mut token) = tokens.next() {
        if token.text == "#" {
            if tokens.peek().is_some_and(|next| next.text == "!") {
                let _ = tokens.next();
            }
            if tokens.peek().is_some_and(|next| next.text == "[") {
                let _ = tokens.next();
                skip_group(&mut tokens);
            }
            continue;
        }
        if token.text == "macro_rules"
            && tokens.peek().is_some_and(|next| next.text == "!")
        {
            // Definitions are not expanded into module-local aliases. Exported
            // macro token trees are independently checked by the surface owner.
            while let Some(next) = tokens.next() {
                if matches!(next.text, "{" | "[" | "(") {
                    skip_group(&mut tokens);
                    break;
                }
            }
            continue;
        }
        let line = token.line;
        let mut public = false;
        if token.text == "pub" {
            public = true;
            if tokens.peek().is_some_and(|next| next.text == "(") {
                public = false;
                let _ = tokens.next();
                skip_group(&mut tokens);
            }
            let Some(next) = tokens.next() else { break };
            token = next;
        }
        let kind = match token.text {
            // `impl Trait + use<T>` is a capture bound, not a use declaration.
            "use" if !tokens.peek().is_some_and(|next| next.text == "<") => {
                StatementKind::Use
            }
            "type" => StatementKind::TypeAlias,
            "extern" if tokens.peek().is_some_and(|next| next.text == "crate") => {
                token = tokens.next().expect("peeked crate token");
                StatementKind::ExternCrate
            }
            _ => continue,
        };
        if statements.len() >= MAX_STATEMENTS {
            return Err(line);
        }
        let (end, complete) = statement_end(&mut tokens, code.len());
        statements.push(Statement {
            kind,
            line,
            public,
            body: &code[token.end..end],
            complete,
        });
    }
    Ok(statements)
}

fn statement_end<'a>(
    tokens: &mut Peekable<impl Iterator<Item = CodeToken<'a>>>,
    eof: usize,
) -> (usize, bool) {
    let mut depth = 0_usize;
    for token in tokens.by_ref() {
        match token.text {
            ";" if depth == 0 => return (token.start, true),
            "(" | "[" | "{" => depth = depth.saturating_add(1),
            ")" | "]" | "}" if depth == 0 => return (token.start, false),
            ")" | "]" | "}" => depth -= 1,
            _ => {}
        }
    }
    (eof, false)
}

fn skip_group<'a>(tokens: &mut impl Iterator<Item = CodeToken<'a>>) {
    let mut depth = 1_usize;
    for token in tokens {
        match token.text {
            "(" | "[" | "{" => depth = depth.saturating_add(1),
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            _ => {}
        }
    }
}

/// Token identity, not an ASCII substring. Full identifier validity remains a
/// compiler responsibility, consistently with the shared conservative tokenizer.
pub(super) fn identifier_name(token: &str) -> Option<&str> {
    let name = token.strip_prefix("r#").unwrap_or(token);
    let first = name.chars().next()?;
    (first.is_ascii_alphabetic()
        || first == '_'
        || (!first.is_ascii() && !first.is_whitespace()))
        .then_some(name)
}

pub(super) fn type_alias_parts<'a>(statement: &Statement<'a>) -> Option<(&'a str, &'a str)> {
    if statement.kind != StatementKind::TypeAlias || !statement.complete {
        return None;
    }
    let name = code_token_spans(statement.body).next()?;
    let alias = identifier_name(name.text)?;
    let definition = &statement.body[name.end..];
    // Include generic defaults and bounds, not only text after the first `=`.
    // A semicolon inside an array/const argument cannot truncate this body.
    code_tokens(definition)
        .any(|token| token == "=")
        .then_some((alias, definition))
}
