//! Direct qualified-path checks for cross-file vendor aliases.
//!
//! `use` propagation is not enough: a public signature can name a restricted
//! alias directly as `crate::private::VendorClient`. This bounded lexer resolves
//! such paths against the semantic module identity built by `module_graph`.

const MAX_PATH_SEGMENTS: usize = 64;
const MAX_QUALIFIED_PATHS: usize = 65_536;

pub(super) fn contains_tainted_qualified_path(
    text: &str,
    current_module: &[String],
    mut is_tainted: impl FnMut(&[String], &str) -> bool,
) -> bool {
    let mut index = 0_usize;
    let mut path_count = 0_usize;

    while index < text.len() {
        let Some((first, first_end)) = read_identifier(text, index) else {
            index = advance_char(text, index);
            continue;
        };

        let mut segments = vec![first];
        let mut cursor = first_end;
        loop {
            let separator = skip_whitespace(text, cursor);
            if !text[separator..].starts_with("::") {
                break;
            }
            let next_start = skip_whitespace(text, separator + 2);
            let Some((next, next_end)) = read_identifier(text, next_start) else {
                break;
            };
            if segments.len() >= MAX_PATH_SEGMENTS {
                return true;
            }
            segments.push(next);
            cursor = next_end;
        }

        if segments.len() >= 2 {
            path_count = path_count.saturating_add(1);
            if path_count > MAX_QUALIFIED_PATHS {
                return true;
            }
            for endpoint in 2..=segments.len() {
                let Some((module, name)) =
                    resolve_item(current_module, &segments[..endpoint])
                else {
                    continue;
                };
                if is_tainted(&module, name) {
                    return true;
                }
            }
        }

        index = cursor.max(advance_char(text, index));
    }

    false
}

fn resolve_item<'a>(
    current_module: &[String],
    path: &'a [String],
) -> Option<(Vec<String>, &'a str)> {
    let (name, prefix) = path.split_last()?;
    if matches!(name.as_str(), "self" | "super" | "crate") {
        return None;
    }

    let mut module = Vec::new();
    let mut index = 0_usize;
    match prefix.first().map(String::as_str) {
        Some("crate") => index = 1,
        Some("self") => {
            module.extend_from_slice(current_module);
            index = 1;
        }
        Some("super") => {
            module.extend_from_slice(current_module);
            while prefix.get(index).map(String::as_str) == Some("super") {
                module.pop()?;
                index += 1;
            }
        }
        _ => module.extend_from_slice(current_module),
    }
    module.extend(prefix[index..].iter().cloned());
    Some((module, name.as_str()))
}

fn read_identifier(text: &str, start: usize) -> Option<(String, usize)> {
    let mut index = skip_whitespace(text, start);
    if text[index..].starts_with("r#") {
        let raw_start = index + 2;
        let first = text[raw_start..].chars().next()?;
        if !identifier_start(first) {
            return None;
        }
        index = raw_start;
    }

    let first = text[index..].chars().next()?;
    if !identifier_start(first) {
        return None;
    }
    let mut end = index + first.len_utf8();
    while let Some(character) = text[end..].chars().next() {
        if !identifier_continue(character) {
            break;
        }
        end += character.len_utf8();
    }
    Some((text[index..end].to_owned(), end))
}

fn skip_whitespace(text: &str, mut index: usize) -> usize {
    while let Some(character) = text[index..].chars().next() {
        if !character.is_whitespace() {
            break;
        }
        index += character.len_utf8();
    }
    index
}

fn advance_char(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map_or(text.len(), |character| index + character.len_utf8())
}

fn identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::contains_tainted_qualified_path;

    fn tainted(module: &[String], name: &str) -> bool {
        module.len() == 1 && module[0] == "private" && name == "VendorClient"
    }

    #[test]
    fn resolves_crate_bare_raw_and_split_paths() {
        for source in [
            "crate::private::VendorClient",
            "private::VendorClient",
            "crate :: r#private :: VendorClient",
            "crate::\nprivate::\nVendorClient",
            "$crate::private::VendorClient",
        ] {
            assert!(
                contains_tainted_qualified_path(source, &[], tainted),
                "{source}"
            );
        }
    }

    #[test]
    fn resolves_super_from_nested_modules() {
        assert!(contains_tainted_qualified_path(
            "super::private::VendorClient",
            &["api".to_owned()],
            tainted,
        ));
    }

    #[test]
    fn checks_tainted_prefix_before_associated_items() {
        assert!(contains_tainted_qualified_path(
            "crate::private::VendorClient::Associated",
            &[],
            tainted,
        ));
    }

    #[test]
    fn unrelated_qualified_paths_remain_clean() {
        for source in [
            "crate::owned::VendorClient",
            "std::collections::BTreeMap",
            "private::Other",
        ] {
            assert!(
                !contains_tainted_qualified_path(source, &[], tainted),
                "{source}"
            );
        }
    }
}
