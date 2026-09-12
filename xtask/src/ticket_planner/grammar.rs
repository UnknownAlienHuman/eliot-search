//! Ticket-planner scalar grammars and closed-field helpers.

use std::collections::BTreeSet;

/// `actor:` identity grammar.
#[must_use]
pub fn actor_identity_valid(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("actor:") else {
        return false;
    };
    let Some((role, identity)) = rest.split_once(':') else {
        return false;
    };
    matches!(role, "user" | "service" | "reviewer" | "integration")
        && opaque_id_valid(identity)
}

/// Package-name grammar.
#[must_use]
pub fn package_name_valid(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut characters = first.chars();
    if !matches!(characters.next(), Some(character) if character.is_ascii_lowercase()) {
        return false;
    }
    if !characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit()
    }) {
        return false;
    }
    for segment in segments {
        if segment.is_empty()
            || !segment.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit()
            })
        {
            return false;
        }
    }
    true
}

/// Algorithm-tagged full Git commit grammar.
#[must_use]
pub fn tagged_git_valid(value: &str) -> bool {
    let Some((algorithm, oid)) = value.split_once(':') else {
        return false;
    };
    let expected = match algorithm {
        "sha1" => 40,
        "sha256" => 64,
        _ => return false,
    };
    oid.len() == expected
        && oid
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Lowercase SHA-256 hex grammar.
#[must_use]
pub fn sha256_hex_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Bounded opaque identifier grammar.
#[must_use]
pub fn opaque_id_valid(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let mut characters = value.chars();
    if !matches!(characters.next(), Some(character) if character.is_ascii_alphanumeric()) {
        return false;
    }
    characters.all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | '-')
    })
}

/// Sorted deduplicated closed-field difference.
#[must_use]
pub fn unknown_fields<'a>(
    keys: &[&'a str],
    allowed: &[&str],
) -> Vec<&'a str> {
    let allowed: BTreeSet<&&str> = allowed.iter().collect();
    let mut unknown: Vec<&'a str> = keys
        .iter()
        .filter(|key| !allowed.contains(key))
        .copied()
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    unknown
}
