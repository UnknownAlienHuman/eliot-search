//! Frozen byte layout for the legacy protected-revision compatibility format.

use super::model::{
    LEGACY_REVISION_INNER_HEADER_BYTES,
    LEGACY_REVISION_MAX_PLAINTEXT_BYTES,
    LEGACY_REVISION_MAX_PLAINTEXT_LENGTH,
    LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES,
    LEGACY_REVISION_OBJECT_VERSION,
    LEGACY_REVISION_OUTER_HEADER_BYTES,
    LegacyRevisionBinding, LegacyRevisionContentDigest,
    LegacyRevisionEnvelopeError, LegacyRevisionExpected,
};

const OUTER_MAGIC: [u8; 8] = *b"ELSRV2\0\0";
const INNER_MAGIC: [u8; 8] = *b"ELSIN2\0\0";

/// Returns whether bytes carry the frozen outer magic.
///
/// Marker presence is not authentication or successful decryption evidence.
#[must_use]
pub fn legacy_revision_is_protected_object(bytes: &[u8]) -> bool {
    bytes.starts_with(&OUTER_MAGIC)
}

/// Encodes the exact authenticated inner envelope around plaintext.
///
/// # Errors
///
/// Returns [`LegacyRevisionEnvelopeError::PlaintextTooLarge`] or
/// [`LegacyRevisionEnvelopeError::LengthMismatch`] when plaintext cannot match
/// the immutable binding.
pub fn encode_legacy_revision_inner(
    binding: LegacyRevisionBinding,
    plaintext: &[u8],
) -> Result<Vec<u8>, LegacyRevisionEnvelopeError> {
    if plaintext.len() > LEGACY_REVISION_MAX_PLAINTEXT_BYTES {
        return Err(LegacyRevisionEnvelopeError::PlaintextTooLarge);
    }
    if u64::try_from(plaintext.len()).ok()
        != Some(binding.revision().plaintext_len())
    {
        return Err(LegacyRevisionEnvelopeError::LengthMismatch);
    }
    let mut output = Vec::with_capacity(
        LEGACY_REVISION_INNER_HEADER_BYTES + plaintext.len(),
    );
    output.extend_from_slice(&INNER_MAGIC);
    output.extend_from_slice(&LEGACY_REVISION_OBJECT_VERSION.to_be_bytes());
    encode_binding(&mut output, binding);
    output.extend_from_slice(plaintext);
    Ok(output)
}

/// Validates and borrows plaintext from an authenticated inner envelope.
///
/// # Errors
///
/// Returns a closed envelope, binding, length, or content mismatch.
pub fn decode_legacy_revision_inner<'a, D: LegacyRevisionContentDigest>(
    inner: &'a [u8],
    expected: LegacyRevisionBinding,
) -> Result<&'a [u8], LegacyRevisionEnvelopeError> {
    if inner.len() < LEGACY_REVISION_INNER_HEADER_BYTES
        || inner.len()
            > LEGACY_REVISION_INNER_HEADER_BYTES
                + LEGACY_REVISION_MAX_PLAINTEXT_BYTES
    {
        return Err(LegacyRevisionEnvelopeError::InnerEnvelopeInvalid);
    }
    let mut cursor = 0;
    if take::<8>(
        inner,
        &mut cursor,
        LegacyRevisionEnvelopeError::InnerEnvelopeInvalid,
    )? != INNER_MAGIC
        || u32::from_be_bytes(take(
            inner,
            &mut cursor,
            LegacyRevisionEnvelopeError::InnerEnvelopeInvalid,
        )?) != LEGACY_REVISION_OBJECT_VERSION
    {
        return Err(LegacyRevisionEnvelopeError::InnerEnvelopeInvalid);
    }
    if decode_binding(
        inner,
        &mut cursor,
        LegacyRevisionEnvelopeError::InnerEnvelopeInvalid,
    )? != expected
    {
        return Err(LegacyRevisionEnvelopeError::InnerBindingMismatch);
    }
    let plaintext_len = usize::try_from(expected.revision().plaintext_len())
        .map_err(|_| LegacyRevisionEnvelopeError::LengthMismatch)?;
    if inner.len().checked_sub(cursor) != Some(plaintext_len) {
        return Err(LegacyRevisionEnvelopeError::LengthMismatch);
    }
    let plaintext = &inner[cursor..];
    if D::digest(plaintext) != expected.revision().content_digest() {
        return Err(LegacyRevisionEnvelopeError::ContentMismatch);
    }
    Ok(plaintext)
}

/// Encodes the exact outer envelope around non-empty protected bytes.
///
/// # Errors
///
/// Returns [`LegacyRevisionEnvelopeError::ProtectedPayloadInvalid`] when the
/// payload is empty or the complete object would exceed its ceiling.
pub fn encode_legacy_revision_outer(
    binding: LegacyRevisionBinding,
    protected: &[u8],
) -> Result<Vec<u8>, LegacyRevisionEnvelopeError> {
    if binding.revision().plaintext_len()
        > LEGACY_REVISION_MAX_PLAINTEXT_LENGTH
    {
        return Err(LegacyRevisionEnvelopeError::PlaintextTooLarge);
    }
    if protected.is_empty()
        || protected.len()
            > LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES
                .saturating_sub(LEGACY_REVISION_OUTER_HEADER_BYTES)
    {
        return Err(LegacyRevisionEnvelopeError::ProtectedPayloadInvalid);
    }
    let protected_len = u64::try_from(protected.len())
        .map_err(|_| LegacyRevisionEnvelopeError::ProtectedPayloadInvalid)?;
    let mut output = Vec::with_capacity(
        LEGACY_REVISION_OUTER_HEADER_BYTES + protected.len(),
    );
    output.extend_from_slice(&OUTER_MAGIC);
    output.extend_from_slice(&LEGACY_REVISION_OBJECT_VERSION.to_be_bytes());
    encode_binding(&mut output, binding);
    output.extend_from_slice(&protected_len.to_be_bytes());
    output.extend_from_slice(protected);
    Ok(output)
}

/// Validates and borrows protected bytes from an exact outer envelope.
///
/// `expected_key_binding` is `None` only when a platform cannot possess the
/// qualified key; all other binding fields remain mandatory.
///
/// # Errors
///
/// Returns a closed framing or binding mismatch.
pub fn decode_legacy_revision_outer(
    object: &[u8],
    expected: LegacyRevisionExpected,
    expected_key_binding: Option<[u8; 32]>,
) -> Result<(LegacyRevisionBinding, &[u8]), LegacyRevisionEnvelopeError> {
    if object.len() < LEGACY_REVISION_OUTER_HEADER_BYTES
        || object.len() > LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES
        || expected.plaintext_len()
            > LEGACY_REVISION_MAX_PLAINTEXT_LENGTH
    {
        return Err(LegacyRevisionEnvelopeError::EnvelopeInvalid);
    }
    let mut cursor = 0;
    if take::<8>(
        object,
        &mut cursor,
        LegacyRevisionEnvelopeError::EnvelopeInvalid,
    )? != OUTER_MAGIC
        || u32::from_be_bytes(take(
            object,
            &mut cursor,
            LegacyRevisionEnvelopeError::EnvelopeInvalid,
        )?) != LEGACY_REVISION_OBJECT_VERSION
    {
        return Err(LegacyRevisionEnvelopeError::EnvelopeInvalid);
    }
    let binding = decode_binding(
        object,
        &mut cursor,
        LegacyRevisionEnvelopeError::EnvelopeInvalid,
    )?;
    if binding.revision().namespace_id() != expected.namespace_id() {
        return Err(LegacyRevisionEnvelopeError::NamespaceMismatch);
    }
    if expected_key_binding
        .is_some_and(|key| key != binding.key_binding_digest())
    {
        return Err(LegacyRevisionEnvelopeError::KeyBindingMismatch);
    }
    if binding.revision() != expected {
        return Err(LegacyRevisionEnvelopeError::EnvelopeBindingMismatch);
    }
    let protected_len = usize::try_from(u64::from_be_bytes(take(
        object,
        &mut cursor,
        LegacyRevisionEnvelopeError::EnvelopeInvalid,
    )?))
    .map_err(|_| LegacyRevisionEnvelopeError::EnvelopeInvalid)?;
    if protected_len == 0
        || object.len().checked_sub(cursor) != Some(protected_len)
    {
        return Err(LegacyRevisionEnvelopeError::EnvelopeInvalid);
    }
    Ok((binding, &object[cursor..]))
}

fn decode_binding(
    bytes: &[u8],
    cursor: &mut usize,
    invalid: LegacyRevisionEnvelopeError,
) -> Result<LegacyRevisionBinding, LegacyRevisionEnvelopeError> {
    let namespace_id = take(bytes, cursor, invalid)?;
    let key_binding_digest = take(bytes, cursor, invalid)?;
    let revision_id = take(bytes, cursor, invalid)?;
    let content_digest = take(bytes, cursor, invalid)?;
    let plaintext_len =
        u64::from_be_bytes(take(bytes, cursor, invalid)?);
    Ok(LegacyRevisionBinding::new(
        LegacyRevisionExpected::new(
            namespace_id,
            revision_id,
            content_digest,
            plaintext_len,
        ),
        key_binding_digest,
    ))
}

fn encode_binding(
    output: &mut Vec<u8>,
    binding: LegacyRevisionBinding,
) {
    output.extend_from_slice(&binding.revision().namespace_id());
    output.extend_from_slice(&binding.key_binding_digest());
    output.extend_from_slice(&binding.revision().revision_id());
    output.extend_from_slice(&binding.revision().content_digest());
    output.extend_from_slice(
        &binding.revision().plaintext_len().to_be_bytes(),
    );
}

fn take<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    invalid: LegacyRevisionEnvelopeError,
) -> Result<[u8; N], LegacyRevisionEnvelopeError> {
    let end = cursor.checked_add(N).ok_or(invalid)?;
    let value = bytes.get(*cursor..end).ok_or(invalid)?;
    *cursor = end;
    value.try_into().map_err(|_| invalid)
}
