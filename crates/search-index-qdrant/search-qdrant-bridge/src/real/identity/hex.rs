fn hex_from_32(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        output.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    output
}

const fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_to_32(text: &str) -> Result<[u8; 32], BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(BridgeError::MalformedResponse);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in bytes.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}
