use super::*;

#[test]
fn frozen_two_row_vector_matches_legacy_chain() {
    let mut chain = SourceImportRecordChain::new();
    chain.push(b"{\"a\":1}\n").expect("row one");
    chain.push(b"{\"b\":2}\n").expect("row two");
    assert_eq!(chain.rows(), 2);
    assert_eq!(chain.encoded_bytes(), 16);
    assert_eq!(
        hex(&chain.finish()),
        "a2a8dc044d640c9c5d91dea46b338425c6d1f457d3eee7693ce0b8da4ee51966"
    );
}

#[test]
fn row_shape_and_bounds_fail_closed_without_advancing() {
    let mut chain = SourceImportRecordChain::new();
    assert_eq!(
        chain.push(b""),
        Err(SourceImportRecordChainError::RowInvalid)
    );
    assert_eq!(
        chain.push(b"missing-newline"),
        Err(SourceImportRecordChainError::RowInvalid)
    );
    assert_eq!(
        chain.push(b"two\nrows\n"),
        Err(SourceImportRecordChainError::RowInvalid)
    );
    assert_eq!(
        chain.push(&vec![b'x'; MAX_SOURCE_IMPORT_ROW_BYTES + 1]),
        Err(SourceImportRecordChainError::RowTooLarge)
    );
    assert_eq!(chain.rows(), 0);
    assert_eq!(chain.encoded_bytes(), 0);
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0f)]));
    }
    output
}
