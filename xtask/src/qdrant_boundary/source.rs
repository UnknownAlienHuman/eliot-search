mod imports;
mod lexer;
mod surface;

#[cfg(test)]
mod tests;

use super::VENDOR_MODULE;
use imports::vendor_identifiers;
use lexer::code_only;
use surface::find_public_vendor_surfaces;

pub(super) fn rust_string_constant(
    source: &str,
    name: &str,
) -> Option<String> {
    let prefix = format!("pub const {name}: &str = ");
    source.lines().find_map(|line| {
        let value = line.trim().strip_prefix(&prefix)?;
        let value = value.strip_suffix(';')?.trim();
        value
            .strip_prefix('"')?
            .strip_suffix('"')
            .map(str::to_owned)
    })
}

pub(super) fn contains_vendor_sdk_reference(source: &str) -> bool {
    let code = code_only(source);
    let path_prefix = format!("{VENDOR_MODULE}::");
    let use_prefix = format!("use {VENDOR_MODULE}");
    let absolute_use_prefix = format!("use ::{VENDOR_MODULE}");
    let extern_prefix = format!("extern crate {VENDOR_MODULE}");
    code.contains(&path_prefix)
        || code.contains(&use_prefix)
        || code.contains(&absolute_use_prefix)
        || code.contains(&extern_prefix)
}

pub(super) fn public_vendor_surface_lines(source: &str) -> Vec<usize> {
    let code = code_only(source);
    let identifiers = vendor_identifiers(&code, VENDOR_MODULE);
    find_public_vendor_surfaces(&code, &identifiers)
}
