use std::collections::BTreeSet;

use super::statements::identifier_name;
use super::tokens::code_tokens;
use super::use_tree::direct_tainted_bindings;

pub(super) fn vendor_identifiers(
    code: &str,
    vendor_module: &str,
) -> BTreeSet<String> {
    let mut identifiers = BTreeSet::from([vendor_module.to_owned()]);
    identifiers.extend(direct_tainted_bindings(code, vendor_module).bindings.into_keys());
    identifiers
}

pub(super) fn contains_any_identifier(
    text: &str,
    identifiers: &BTreeSet<String>,
) -> bool {
    code_tokens(text).filter_map(reference_name)
        .any(|name| identifiers.contains(name))
}

pub(super) fn reference_name(token: &str) -> Option<&str> {
    if !token.starts_with("r#") && reserved_word(token) {
        return None;
    }
    identifier_name(token)
}

// Raw aliases such as r#type must not taint the unrelated `type` keyword in
// every public type declaration. Weak keywords remain possible ordinary names.
fn reserved_word(token: &str) -> bool {
    matches!(token,
        "_" | "as" | "async" | "await" | "break" | "const" | "continue"
        | "crate" | "dyn" | "else" | "enum" | "extern" | "false" | "fn"
        | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod"
        | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
        | "static" | "struct" | "super" | "trait" | "true" | "type"
        | "unsafe" | "use" | "where" | "while" | "abstract" | "become"
        | "box" | "do" | "final" | "gen" | "macro" | "override" | "priv"
        | "try" | "typeof" | "unsized" | "virtual" | "yield"
    )
}
