//! Markdown heading extraction used by coverage tooling.

/// One Markdown heading row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// One-based line number.
    pub line: usize,
    /// Heading level 1 through 4.
    pub level: u8,
    /// Raw heading text.
    pub raw: String,
    /// Heading text with inline emphasis markers removed.
    pub title: String,
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let bytes = line.as_bytes();
    let mut hashes = 0_usize;
    while hashes < bytes.len() && bytes[hashes] == b'#' {
        hashes += 1;
    }
    if hashes == 0 || hashes > 4 {
        return None;
    }
    let rest = &line[hashes..];
    let first = rest.chars().next()?;
    if !first.is_whitespace() {
        return None;
    }
    let stripped = rest[first.len_utf8()..].trim();
    if stripped.is_empty() {
        return None;
    }
    Some((u8::try_from(hashes).ok()?, stripped.to_owned()))
}

/// Extracts `#` through `####` Markdown headings with historical normalization.
#[must_use]
pub fn heading_rows(text: &str) -> Vec<Heading> {
    let mut output = Vec::new();
    for (index, raw_line) in text.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let Some((level, raw)) = parse_heading(line) else {
            continue;
        };
        let title = raw
            .chars()
            .filter(|character| !matches!(character, '`' | '*' | '_'))
            .collect::<String>()
            .trim()
            .to_owned();
        output.push(Heading {
            line: index + 1,
            level,
            raw,
            title,
        });
    }
    output
}
