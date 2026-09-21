mod cross_file;
mod imports;
mod lexer;
mod module_graph;
mod statements;
mod surface;
mod tokens;
mod use_tree;

#[cfg(test)]
mod import_regressions;

#[cfg(test)]
mod surface_regressions;

#[cfg(test)]
mod tests;

pub(super) use cross_file::{
    BridgeSource, find_cross_file_vendor_surfaces,
};

use super::VENDOR_MODULE;
use imports::vendor_identifiers;
use lexer::code_only;
use surface::find_public_vendor_surfaces;
use tokens::code_tokens;

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

pub(super) fn public_vendor_surface_lines(source: &str) -> Vec<usize> {
    let code = code_only(source);
    let identifiers = vendor_identifiers(&code, VENDOR_MODULE);
    find_public_vendor_surfaces(&code, &identifiers)
}
