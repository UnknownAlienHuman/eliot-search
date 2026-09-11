//! Bounded port of pure `tools/coverage_graph_v2.py` helpers (T41, family F).
//!
//! Covers only deterministic string helpers (`slug`, `words`, `q`/`arr`,
//! `digest_text`, `heading_rows`, `replace_once`, `module_refs_to_packages`).
//! Graph derivation (`build_graph`), TOML renders, manifest patching and both
//! `generate`/`validate` entrypoints remain Python-owned (see report).

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Typed failure for `replace_once` (`RuntimeError` in Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageGraphError(String);

impl CoverageGraphError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for CoverageGraphError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

impl Error for CoverageGraphError {}

/// `digest_text`: SHA-256 hex of UTF-8 bytes (`hashlib.sha256(...).hexdigest()`).
#[must_use]
pub fn digest_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// `slug`: lowercase, `[^a-z0-9]+` runs to `-`, strip `-`, truncate 96, or `"node"`.
#[must_use]
pub fn slug(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut need_dash = false;
    let mut has_out = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if need_dash && has_out {
                out.push('-');
            }
            need_dash = false;
            has_out = true;
            out.push(c);
        } else {
            need_dash = true;
        }
    }
    if out.len() > 96 {
        out.truncate(96);
    }
    if out.is_empty() {
        return String::from("node");
    }
    out
}

fn append_json_string(out: &mut Vec<u8>, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(b'"');
    for c in value.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(b"\\u00");
                let b = c as u8;
                out.push(HEX[usize::from(b >> 4)]);
                out.push(HEX[usize::from(b & 0x0F)]);
            }
            c => {
                let mut buf = [0_u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}

/// `q`: `json.dumps(value, ensure_ascii=False)` for strings.
///
/// # Panics
///
/// Panics only on internal UTF-8 invariant violation (unreachable: the
/// builder emits ASCII escapes plus raw UTF-8).
#[must_use]
pub fn quote_json(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len() + 2);
    append_json_string(&mut out, value);
    String::from_utf8(out).expect("json quoting emits valid UTF-8")
}

/// `arr`: `"[" + ", ".join(q(v)) + "]"`.
#[must_use]
pub fn arr(values: &[&str]) -> String {
    let mut out = String::from("[");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&quote_json(v));
    }
    out.push(']');
    out
}

/// `words`: camel-split, lowercase `[a-z0-9]+` tokens plus `ies`/`s`/`ing`/`ed` stems.
#[must_use]
pub fn words(value: &str) -> BTreeSet<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut spaced = String::with_capacity(value.len() + 4);
    for (i, c) in chars.iter().enumerate() {
        if i > 0
            && c.is_ascii_uppercase()
            && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit())
        {
            spaced.push(' ');
        }
        spaced.push(*c);
    }
    let lower = spaced.to_lowercase();
    let normalized: String = lower
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in normalized.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let mut out = BTreeSet::new();
    for token in tokens {
        if token.ends_with("ies") && token.len() > 4 {
            out.insert(format!("{}y", &token[..token.len() - 3]));
        } else if token.ends_with('s') && token.len() > 3 {
            out.insert(token[..token.len() - 1].to_owned());
        }
        if token.ends_with("ing") && token.len() > 5 {
            out.insert(token[..token.len() - 3].to_owned());
        }
        if token.ends_with("ed") && token.len() > 4 {
            out.insert(token[..token.len() - 2].to_owned());
        }
        out.insert(token);
    }
    out
}

/// One Markdown heading row (`line`, 1-based `level`, `raw`, `title`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// 1-based line number.
    pub line: usize,
    /// Heading level 1..=4.
    pub level: u8,
    /// Raw heading text (trailing whitespace stripped).
    pub raw: String,
    /// Title with `` ` ``, `*`, `_` removed and trimmed.
    pub title: String,
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let bytes = line.as_bytes();
    let mut hashes: usize = 0;
    while hashes < bytes.len() && bytes[hashes] == b'#' {
        hashes += 1;
    }
    if hashes == 0 || hashes > 4 {
        return None;
    }
    let rest = &line[hashes..];
    let mut rest_chars = rest.chars();
    let first = rest_chars.next()?;
    if !first.is_whitespace() {
        return None;
    }
    let after = &rest[first.len_utf8()..];
    // `(.+?)\s*$`: require non-empty after separators, strip trailing space.
    let stripped = after.trim_start().trim_end();
    if stripped.is_empty() {
        // All-whitespace remainder: Python still matches when `rest` holds at
        // least two whitespace chars (`\s+` takes L-1, `(.+?)` takes the last).
        let chars: Vec<char> = rest.chars().collect();
        if chars.len() >= 2 && chars.iter().all(|c| c.is_whitespace()) {
            let level = u8::try_from(hashes).ok()?;
            let raw = chars.last().map_or_else(String::new, ToString::to_string);
            return Some((level, raw));
        }
        return None;
    }
    let level = u8::try_from(hashes).ok()?;
    Some((level, stripped.to_owned()))
}

/// `heading_rows`: `^(#{1,4})\s+(.+?)\s*$` per line with `` `*_ ``-stripped title.
#[must_use]
pub fn heading_rows(text: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    for (idx, raw_line) in text.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let Some((level, raw)) = parse_heading(line) else {
            continue;
        };
        let title: String = raw
            .chars()
            .filter(|c| *c != '`' && *c != '*' && *c != '_')
            .collect::<String>()
            .trim()
            .to_owned();
        out.push(Heading {
            line: idx + 1,
            level,
            raw,
            title,
        });
    }
    out
}

/// `replace_once` from `generate-coverage-graph-v2.py`.
///
/// # Errors
///
/// Returns `CoverageGraphError` when `old` occurs zero or multiple times.
pub fn replace_once(
    text: &str,
    old: &str,
    new: &str,
    label: &str,
) -> Result<String, CoverageGraphError> {
    let count = text.matches(old).count();
    if count != 1 {
        return Err(CoverageGraphError::new(format!(
            "{label}: expected one occurrence, found {count}"
        )));
    }
    Ok(text.replacen(old, new, 1))
}

/// `module_refs_to_packages`: sorted unique `pkg` from `pkg:module` refs.
#[must_use]
pub fn module_refs_to_packages(refs: &[&str]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for r in refs {
        let pkg = match r.split_once(':') {
            Some((pkg, _)) => pkg,
            None => *r,
        };
        set.insert(pkg.to_owned());
    }
    set.into_iter().collect()
}
