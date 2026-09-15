mod cross_file;
mod imports;
mod lexer;
mod surface;

#[cfg(test)]
mod tests;

pub(super) use cross_file::{
    BridgeSource, find_cross_file_vendor_surfaces,
};

use super::VENDOR_MODULE;
use imports::vendor_identifiers;
use lexer::code_only;
use surface::find_public_vendor_surfaces;

pub(super) fn rust_string_constant(
    source: &str,
    name: &str,
) -> Option<String> {
    let prefix = format!("pub const {name}: &str = ");
    let code = code_only(source);
    // Only active code may provide a qualification identity. Keep the existing
    // canonical, single-line literal format and reject ambiguous definitions.
    let mut values = source.lines().zip(code.lines()).filter_map(
        |(line, code_line)| {
            code_line.trim_start().strip_prefix(&prefix)?;
            let value = line.trim().strip_prefix(&prefix)?;
            let value = value.strip_suffix(';')?.trim();
            value
                .strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_owned)
        },
    );
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

pub(super) fn contains_vendor_sdk_reference(source: &str) -> bool {
    let code = code_only(source);
    let mut previous = ["", ""];
    let mut in_import = false;
    for token in code_tokens(&code) {
        let is_vendor = token.strip_prefix("r#").unwrap_or(token) == VENDOR_MODULE;
        if is_vendor && (in_import || previous == ["extern", "crate"]) {
            return true;
        }
        if token == ":" && previous == [VENDOR_MODULE, ":"] {
            return true;
        }
        match token {
            "use" => in_import = true,
            ";" => in_import = false,
            _ => {}
        }
        previous = [previous[1], if is_vendor { VENDOR_MODULE } else { token }];
    }
    false
}

// Token boundaries, rather than textual `use ` / `crate::` prefixes, make
// comments, whitespace and raw identifiers immaterial to a reference. This
// streams over the masked source without constructing another token buffer.
fn code_tokens(mut code: &str) -> impl Iterator<Item = &str> {
    std::iter::from_fn(move || {
        code = code.trim_start();
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
        let (token, rest) = code.split_at(raw_prefix + end);
        code = rest;
        Some(token)
    })
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '_'
        || (!character.is_ascii() && !character.is_whitespace())
}

pub(super) fn public_vendor_surface_lines(source: &str) -> Vec<usize> {
    let code = code_only(source);
    let identifiers = vendor_identifiers(&code, VENDOR_MODULE);
    find_public_vendor_surfaces(&code, &identifiers)
}
