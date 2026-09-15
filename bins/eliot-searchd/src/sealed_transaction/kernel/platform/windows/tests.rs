use crate::sealed_digest::Sha256Digest;

use super::codec::{
    RECEIPT_MAGIC, encode_receipt, field, parse_metadata, parse_u64,
};
use super::model::Receipt;

#[test]
fn receipt_encoding_retains_the_exact_v2_wire_bytes() {
    let digest = Sha256Digest::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    let receipt = Receipt {
        operation_id: "operation-1".to_owned(),
        object_id: "object-1".to_owned(),
        plaintext_bytes: 12,
        plaintext_sha256: digest,
        ciphertext_bytes: 256,
    };
    let encoded = encode_receipt(&receipt);
    assert_eq!(
        encoded,
        concat!(
            "ELIOT-SEALED-RECEIPT-V1\n",
            "operation=operation-1\n",
            "object=object-1\n",
            "plaintext_bytes=12\n",
            "plaintext_sha256=0000000000000000000000000000000000000000000000000000000000000000\n",
            "ciphertext_bytes=256\n",
        )
    );
    let fields = parse_metadata(&encoded, RECEIPT_MAGIC).unwrap();
    assert_eq!(fields.len(), 5);
    assert_eq!(
        parse_u64(field(&fields, "plaintext_bytes").unwrap()).unwrap(),
        12
    );
}
