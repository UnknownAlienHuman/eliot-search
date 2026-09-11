pub(super) fn code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = vec![b' '; bytes.len()];
    let mut index = 0_usize;
    let mut state = LexState::Code;

    while index < bytes.len() {
        match state {
            LexState::Code => {
                if bytes[index..].starts_with(b"//") {
                    state = LexState::LineComment;
                    index += 2;
                } else if bytes[index..].starts_with(b"/*") {
                    state = LexState::BlockComment(1);
                    index += 2;
                } else if let Some((content_start, hashes)) =
                    raw_string_start(bytes, index)
                {
                    state = LexState::RawString(hashes);
                    index = content_start;
                } else if bytes[index] == b'"' {
                    state = LexState::String(false);
                    index += 1;
                } else if bytes[index] == b'\'' {
                    if let Some(end) = char_literal_end(bytes, index) {
                        index = end;
                    } else {
                        output[index] = bytes[index];
                        index += 1;
                    }
                } else {
                    output[index] = bytes[index];
                    index += 1;
                }
            }
            LexState::LineComment => {
                if bytes[index] == b'\n' {
                    output[index] = b'\n';
                    state = LexState::Code;
                }
                index += 1;
            }
            LexState::BlockComment(depth) => {
                if bytes[index] == b'\n' {
                    output[index] = b'\n';
                    index += 1;
                } else if bytes[index..].starts_with(b"/*") {
                    state = LexState::BlockComment(depth.saturating_add(1));
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    if depth == 1 {
                        state = LexState::Code;
                    } else {
                        state = LexState::BlockComment(depth - 1);
                    }
                    index += 2;
                } else {
                    index += 1;
                }
            }
            LexState::String(escaped) => {
                if bytes[index] == b'\n' {
                    output[index] = b'\n';
                }
                if escaped {
                    state = LexState::String(false);
                } else if bytes[index] == b'\\' {
                    state = LexState::String(true);
                } else if bytes[index] == b'"' {
                    state = LexState::Code;
                }
                index += 1;
            }
            LexState::RawString(hashes) => {
                if bytes[index] == b'\n' {
                    output[index] = b'\n';
                    index += 1;
                } else if raw_string_end(bytes, index, hashes) {
                    index += 1 + hashes;
                    state = LexState::Code;
                } else {
                    index += 1;
                }
            }
        }
    }

    String::from_utf8(output)
        .expect("replacing source bytes with ASCII spaces preserves UTF-8")
}

#[derive(Clone, Copy, Debug)]
enum LexState {
    Code,
    LineComment,
    BlockComment(usize),
    String(bool),
    RawString(usize),
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let mut cursor = index;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0_usize;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor + 1, hashes))
    } else {
        None
    }
}

fn raw_string_end(bytes: &[u8], index: usize, hashes: usize) -> bool {
    bytes.get(index) == Some(&b'"')
        && (0..hashes).all(|offset| {
            bytes.get(index + 1 + offset) == Some(&b'#')
        })
}

fn char_literal_end(bytes: &[u8], index: usize) -> Option<usize> {
    let first = *bytes.get(index + 1)?;
    if matches!(first, b'\n' | b'\r') {
        return None;
    }
    if first == b'\\' {
        let mut cursor = index + 1;
        let mut escaped = false;
        while cursor < bytes.len() && cursor <= index + 16 {
            match bytes[cursor] {
                b'\n' | b'\r' => return None,
                b'\'' if !escaped => return Some(cursor + 1),
                b'\\' if !escaped => escaped = true,
                _ => escaped = false,
            }
            cursor += 1;
        }
        return None;
    }

    let width = utf8_char_width(first)?;
    let closing = index + 1 + width;
    (bytes.get(closing) == Some(&b'\'')).then_some(closing + 1)
}

const fn utf8_char_width(first: u8) -> Option<usize> {
    if first < 0x80 {
        Some(1)
    } else if first & 0xE0 == 0xC0 {
        Some(2)
    } else if first & 0xF0 == 0xE0 {
        Some(3)
    } else if first & 0xF8 == 0xF0 {
        Some(4)
    } else {
        None
    }
}
