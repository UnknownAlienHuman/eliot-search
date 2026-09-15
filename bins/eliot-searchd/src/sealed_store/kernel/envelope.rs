//! Strict bounded persisted-envelope codec.

use super::spec::{
    FORMAT_VERSION, HEADER_BYTES, MAGIC, MAX_ENVELOPE_BYTES,
    MAX_OBJECT_ID_BYTES, MAX_PLAINTEXT_BYTES, SealedStoreError,
    validate_object_id,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Envelope {
    pub(crate) object_id: String,
    pub(crate) plaintext_bytes: u64,
    pub(crate) ciphertext: Vec<u8>,
}

impl Envelope {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, SealedStoreError> {
        validate_object_id(&self.object_id)?;
        let id = self.object_id.as_bytes();
        let id_len =
            u16::try_from(id.len()).map_err(|_| SealedStoreError::InvalidObjectId)?;
        let ciphertext_len = u64::try_from(self.ciphertext.len())
            .map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
        let total = HEADER_BYTES
            .checked_add(id.len())
            .and_then(|value| value.checked_add(self.ciphertext.len()))
            .ok_or(SealedStoreError::EnvelopeTooLarge)?;
        if total > MAX_ENVELOPE_BYTES || self.ciphertext.is_empty() {
            return Err(SealedStoreError::EnvelopeTooLarge);
        }
        let mut output = Vec::with_capacity(total);
        output.extend_from_slice(&MAGIC);
        output.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        output.extend_from_slice(&id_len.to_be_bytes());
        output.extend_from_slice(&self.plaintext_bytes.to_be_bytes());
        output.extend_from_slice(&ciphertext_len.to_be_bytes());
        output.extend_from_slice(id);
        output.extend_from_slice(&self.ciphertext);
        Ok(output)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, SealedStoreError> {
        if bytes.len() < HEADER_BYTES {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        if bytes.len() > MAX_ENVELOPE_BYTES || bytes[..8] != MAGIC {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let version = u16::from_be_bytes(
            bytes[8..10]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        );
        if version != FORMAT_VERSION {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let id_len = usize::from(u16::from_be_bytes(
            bytes[10..12]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        ));
        let plaintext_bytes = u64::from_be_bytes(
            bytes[12..20]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        );
        let ciphertext_len = usize::try_from(u64::from_be_bytes(
            bytes[20..28]
                .try_into()
                .map_err(|_| SealedStoreError::EnvelopeInvalid)?,
        ))
        .map_err(|_| SealedStoreError::EnvelopeTooLarge)?;
        let ciphertext_start = HEADER_BYTES
            .checked_add(id_len)
            .ok_or(SealedStoreError::EnvelopeInvalid)?;
        let expected = ciphertext_start
            .checked_add(ciphertext_len)
            .ok_or(SealedStoreError::EnvelopeTooLarge)?;
        if id_len == 0
            || id_len > MAX_OBJECT_ID_BYTES
            || ciphertext_len == 0
            || expected != bytes.len()
            || plaintext_bytes == 0
            || plaintext_bytes
                > u64::try_from(MAX_PLAINTEXT_BYTES).unwrap_or(u64::MAX)
        {
            return Err(SealedStoreError::EnvelopeInvalid);
        }
        let object_id = core::str::from_utf8(
            &bytes[HEADER_BYTES..ciphertext_start],
        )
        .map_err(|_| SealedStoreError::EnvelopeInvalid)?
        .to_owned();
        validate_object_id(&object_id)?;
        Ok(Self {
            object_id,
            plaintext_bytes,
            ciphertext: bytes[ciphertext_start..].to_vec(),
        })
    }
}
