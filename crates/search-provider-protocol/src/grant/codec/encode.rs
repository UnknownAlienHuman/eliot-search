//! Canonical standalone-grant request encoding.

use search_contracts::CorpusOrPortfolioId;

use crate::error::ProtocolError;

use super::super::{
    MAX_STANDALONE_GRANT_REQUEST_BYTES, STANDALONE_GRANT_REQUEST_VERSION,
    StandaloneGrantRequestV1,
};

/// Encodes the exact canonical UTF-8 JSON grant-request body.
///
/// Field order, numeric spelling, lower-case hex, set order and enum wire
/// spellings are fixed. Profile bytes are lower-case hexadecimal UTF-8 so no
/// alternate JSON escaping can represent the same profile identity.
///
/// # Errors
///
/// Returns a typed protocol error when the request shape or body size is
/// invalid.
pub fn encode_standalone_grant_request(
    request: &StandaloneGrantRequestV1,
) -> Result<Vec<u8>, ProtocolError> {
    request.validate()?;
    let mut output = Vec::with_capacity(1024);
    output.extend_from_slice(b"{\"v\":");
    push_u64(&mut output, u64::from(STANDALONE_GRANT_REQUEST_VERSION));
    output.extend_from_slice(b",\"binding_generation\":");
    push_u64(&mut output, request.expected_binding_generation);
    output.extend_from_slice(b",\"policy_generation\":");
    push_u64(&mut output, request.expected_policy_generation);
    output.extend_from_slice(b",\"memberships\":");
    push_array(
        &mut output,
        request.requested_membership_ids.iter(),
        |out, value| push_quoted_hex(out, value.as_bytes()),
    );
    output.extend_from_slice(b",\"targets\":");
    push_array(
        &mut output,
        request.requested_corpus_or_portfolio_ids.iter(),
        |out, value| {
            out.push(b'"');
            match value {
                CorpusOrPortfolioId::Corpus(id) => {
                    out.extend_from_slice(b"c:");
                    push_hex(out, id.as_bytes());
                }
                CorpusOrPortfolioId::Portfolio(id) => {
                    out.extend_from_slice(b"p:");
                    push_hex(out, id.as_bytes());
                }
            }
            out.push(b'"');
        },
    );
    output.extend_from_slice(b",\"partitions\":");
    push_array(
        &mut output,
        request.requested_access_partitions.iter(),
        |out, value| push_quoted_hex(out, value.as_bytes()),
    );
    output.extend_from_slice(b",\"modalities\":");
    push_array(
        &mut output,
        request.requested_modalities.iter(),
        |out, value| push_quoted_ascii(out, value.as_str().as_bytes()),
    );
    output.extend_from_slice(b",\"recipes\":");
    push_array(
        &mut output,
        request.requested_recipe_families.iter(),
        |out, value| push_quoted_ascii(out, value.as_str().as_bytes()),
    );
    output.extend_from_slice(b",\"budget_hex\":\"");
    push_hex(&mut output, request.requested_budget_class.as_str().as_bytes());
    output.extend_from_slice(b"\",\"sensitivity\":\"");
    output.extend_from_slice(request.requested_sensitivity_ceiling.as_str().as_bytes());
    output.extend_from_slice(b"\",\"disclosure\":\"");
    output.extend_from_slice(request.requested_disclosure_ceiling.as_str().as_bytes());
    output.extend_from_slice(b"\",\"source_read\":");
    push_bool(&mut output, request.requested_source_read_permission);
    output.extend_from_slice(b",\"exact_scan\":");
    push_bool(&mut output, request.requested_exact_scan_permission);
    output.extend_from_slice(b",\"ttl_ms\":");
    push_u64(&mut output, request.requested_ttl_ms);
    output.push(b'}');
    if output.len() > MAX_STANDALONE_GRANT_REQUEST_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    Ok(output)
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(value.to_string().as_bytes());
}

fn push_bool(output: &mut Vec<u8>, value: bool) {
    output.extend_from_slice(if value { &b"true"[..] } else { &b"false"[..] });
}

fn push_array<'a, T: 'a>(
    output: &mut Vec<u8>,
    items: impl IntoIterator<Item = &'a T>,
    mut push_item: impl FnMut(&mut Vec<u8>, &T),
) {
    output.push(b'[');
    let mut first = true;
    for item in items {
        if !first {
            output.push(b',');
        }
        first = false;
        push_item(output, item);
    }
    output.push(b']');
}

fn push_quoted_hex(output: &mut Vec<u8>, bytes: &[u8]) {
    output.push(b'"');
    push_hex(output, bytes);
    output.push(b'"');
}

fn push_quoted_ascii(output: &mut Vec<u8>, bytes: &[u8]) {
    output.push(b'"');
    output.extend_from_slice(bytes);
    output.push(b'"');
}

fn push_hex(output: &mut Vec<u8>, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)]);
        output.push(HEX[usize::from(byte & 0x0f)]);
    }
}
