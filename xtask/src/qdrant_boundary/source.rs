use super::VENDOR_MODULE;

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
    let path_prefix = format!("{VENDOR_MODULE}::");
    let use_prefix = format!("use {VENDOR_MODULE}");
    let extern_prefix = format!("extern crate {VENDOR_MODULE}");
    source.contains(&path_prefix)
        || source.contains(&use_prefix)
        || source.contains(&extern_prefix)
}

pub(super) fn public_vendor_surface_lines(source: &str) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut public_signature: Option<(usize, String)> = None;
    let public_use_prefix = format!("pub use {VENDOR_MODULE}");
    let public_extern_prefix = format!("pub extern crate {VENDOR_MODULE}");

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if public_signature.is_none() && starts_public_signature(trimmed) {
            public_signature = Some((line_number, String::new()));
        }

        let mut clear_signature = false;
        if let Some((start, signature)) = public_signature.as_mut() {
            signature.push_str(trimmed);
            signature.push('\n');
            if signature.contains(VENDOR_MODULE) {
                violations.push(*start);
                clear_signature = true;
            } else if signature_ended(trimmed) {
                clear_signature = true;
            }
        }
        if clear_signature {
            public_signature = None;
        }

        if trimmed.starts_with(&public_use_prefix)
            || trimmed.starts_with(&public_extern_prefix)
            || (trimmed.starts_with("pub ")
                && trimmed.contains(VENDOR_MODULE))
        {
            violations.push(line_number);
        }
    }

    violations.sort_unstable();
    violations.dedup();
    violations
}

fn starts_public_signature(line: &str) -> bool {
    line.starts_with("pub fn ")
        || line.starts_with("pub async fn ")
        || line.starts_with("pub unsafe fn ")
        || line.starts_with("pub const fn ")
        || line.starts_with("pub type ")
        || line.starts_with("pub static ")
}

fn signature_ended(line: &str) -> bool {
    line.contains('{') || line.ends_with(';')
}
