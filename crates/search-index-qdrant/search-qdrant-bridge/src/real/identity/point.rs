/// Provider-neutral 128-bit point ID rendered as a lowercase UUID string for
/// the vendor transport (the full 128 bits survive the round trip).
fn uuid_string(id: &QdrantPointId) -> String {
    let mut hex = String::with_capacity(32);
    for byte in id.0 {
        hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        hex.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn parse_uuid(text: &str) -> Result<QdrantPointId, BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return Err(BridgeError::MalformedResponse);
    }
    for dash in [8, 13, 18, 23] {
        if bytes[dash] != b'-' {
            return Err(BridgeError::MalformedResponse);
        }
    }
    let mut compact = [0_u8; 32];
    let mut next = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        compact[next] = *byte;
        next += 1;
    }
    let mut output = [0_u8; 16];
    for (index, pair) in compact.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        output[index] = (high << 4) | low;
    }
    Ok(QdrantPointId(output))
}

fn vendor_point_id(id: &QdrantPointId) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Uuid(uuid_string(id))),
    }
}

fn bridge_point_id(id: &PointId) -> Result<QdrantPointId, BridgeError> {
    match id.point_id_options.as_ref() {
        Some(point_id::PointIdOptions::Num(number)) => {
            let mut bytes = [0_u8; 16];
            bytes[8..16].copy_from_slice(&number.to_be_bytes());
            Ok(QdrantPointId(bytes))
        }
        Some(point_id::PointIdOptions::Uuid(text)) => parse_uuid(text),
        None => Err(BridgeError::MalformedResponse),
    }
}
