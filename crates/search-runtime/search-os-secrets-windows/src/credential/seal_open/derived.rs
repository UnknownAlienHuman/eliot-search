//! Closed derived-artifact key use over the same finite selected-key window.

use core::fmt;
use search_control_redb::source_retention::{
    CommittedDerivedArtifactSealUse, DerivedArtifactReservationReadback,
};
use search_os_secrets::{RevisionDekCurrent, ValidatedSecretsConfig};
use search_revision_crypto::{
    DerivedArtifactCryptoErrorV1, DerivedArtifactPayloadBindingV1,
    DerivedArtifactPayloadPlaintextV1, DerivedArtifactSelectedMetadataV1, REVISION_TAG_BYTES,
    open_derived_artifact_payload_v1, seal_derived_artifact_payload_v1,
};
use search_revision_store::{
    derived_artifact::{
        DerivedArtifactSealBindingV1, MAX_DERIVED_AAD_BYTES, MAX_DERIVED_CURRENT_BYTES,
        MAX_DERIVED_DESCRIPTOR_BYTES,
    },
    residency_canonical_bytes,
};

use super::{KeyWindow, RevisionSealOpenError, Work, current, finish};
use crate::{
    LegacyRevisionDpapiContext, RevisionDekContext, RevisionDekError, read_revision_dek_key,
};

/// Content-free refusal of a finite derived-artifact seal/open operation.
pub enum DerivedArtifactSealOpenError<E> {
    /// Original context, selected backend, key window or exact native key refused.
    Key(RevisionSealOpenError<E>),
    /// C07 metadata differs from the exact C02 reservation or selected key.
    BindingMismatch,
    /// The existing pure cryptographic owner refused authentication or content.
    Crypto(DerivedArtifactCryptoErrorV1),
    /// A finite logical allocation/processing bound could not be represented.
    SizeLimit,
}
impl<E> DerivedArtifactSealOpenError<E> {
    /// Stable reason without key, source, native error or metadata bytes.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Key(error) => error.code(),
            Self::BindingMismatch => "DERIVED_ARTIFACT_SEAL_OPEN_BINDING_MISMATCH",
            Self::Crypto(_) => "DERIVED_ARTIFACT_SEAL_OPEN_CRYPTO_FAILED",
            Self::SizeLimit => "DERIVED_ARTIFACT_SEAL_OPEN_SIZE_LIMIT",
        }
    }
}
impl<E> fmt::Debug for DerivedArtifactSealOpenError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl<E> fmt::Display for DerivedArtifactSealOpenError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

// These borrows convey no capability. Seal requires the owned fresh permit;
// open requires the C02-owned exact reservation readback, never a decoded token.
struct Reserved<'a> {
    descriptor: &'a [u8],
    residency: &'a [u8],
    current: &'a [u8],
    aad: &'a [u8],
    preparation: &'a [u8],
    counter: u64,
    length: u64,
    blocks: u64,
}
impl<'a> From<&'a CommittedDerivedArtifactSealUse> for Reserved<'a> {
    fn from(value: &'a CommittedDerivedArtifactSealUse) -> Self {
        Self {
            descriptor: value.descriptor_canonical(),
            residency: value.residency_canonical(),
            current: value.selected_current_canonical(),
            aad: value.aad_canonical(),
            preparation: value.preparation_canonical(),
            counter: value.counter(),
            length: value.plaintext_length(),
            blocks: value.authenticated_blocks(),
        }
    }
}
impl<'a> From<&'a DerivedArtifactReservationReadback> for Reserved<'a> {
    fn from(value: &'a DerivedArtifactReservationReadback) -> Self {
        Self {
            descriptor: value.descriptor_canonical(),
            residency: value.residency_canonical(),
            current: value.selected_current_canonical(),
            aad: value.aad_canonical(),
            preparation: value.preparation_canonical(),
            counter: value.counter(),
            length: value.plaintext_length(),
            blocks: value.authenticated_blocks(),
        }
    }
}

fn reserve<C: RevisionDekContext>(
    window: &mut KeyWindow<'_, C>,
    body: usize,
    ciphertext: usize,
) -> Result<(), DerivedArtifactSealOpenError<C::Error>> {
    // Prepay descriptor/residency encoding, Current decoding and pure-crypto
    // copies before those bounded allocations. All charges use the same caller.
    let metadata =
        (MAX_DERIVED_DESCRIPTOR_BYTES + MAX_DERIVED_CURRENT_BYTES + MAX_DERIVED_AAD_BYTES + 8192)
            * 16;
    let allocation = body
        .checked_mul(6)
        .and_then(|n| ciphertext.checked_mul(3).and_then(|c| n.checked_add(c)))
        .and_then(|n| n.checked_add(metadata))
        .ok_or(DerivedArtifactSealOpenError::SizeLimit)?;
    let processing = allocation
        .checked_add(body)
        .and_then(|n| n.checked_add(ciphertext))
        .ok_or(DerivedArtifactSealOpenError::SizeLimit)?;
    for work in [Work::Allocate(allocation), Work::Process(processing)] {
        window.charge(work).map_err(|error| {
            DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Context(error))
        })?;
    }
    Ok(())
}

fn binding<E>(
    owner: &DerivedArtifactSealBindingV1,
    reserved: &Reserved<'_>,
    config: &ValidatedSecretsConfig,
) -> Result<(DerivedArtifactPayloadBindingV1, RevisionDekCurrent), DerivedArtifactSealOpenError<E>>
{
    let mismatch = || DerivedArtifactSealOpenError::BindingMismatch;
    let descriptor = owner.descriptor();
    if descriptor.encode().map_err(|_| mismatch())? != reserved.descriptor
        || residency_canonical_bytes(descriptor.domains()).map_err(|_| mismatch())?
            != reserved.residency
        || owner.selected_current_canonical() != reserved.current
        || owner.aad_bytes() != reserved.aad
        || owner.counter() != reserved.counter
        || descriptor.encoded_body_bytes() != reserved.length
        || owner.authenticated_blocks().map_err(|_| mismatch())? != reserved.blocks
    {
        return Err(mismatch());
    }
    let current = current(reserved.current, config).map_err(DerivedArtifactSealOpenError::Key)?;
    let selected = current.intent().candidate().intent().binding();
    if selected.key_reference().map_err(|_| mismatch())? != owner.encryption().key_reference
        || selected.key_version() != owner.encryption().key_version.get()
        || selected.protection_entropy() != reserved.preparation
    {
        return Err(mismatch());
    }
    let metadata = DerivedArtifactSelectedMetadataV1::new(
        reserved.current,
        owner.encryption().key_reference.clone(),
        owner.encryption().key_version,
    )
    .map_err(DerivedArtifactSealOpenError::Crypto)?;
    let expected = DerivedArtifactPayloadBindingV1::from_c07_aad(
        owner.aad_bytes(),
        metadata,
        owner.counter(),
        *owner.nonce(),
        descriptor.encoded_body_digest(),
        descriptor.encoded_body_bytes(),
    )
    .map_err(DerivedArtifactSealOpenError::Crypto)?;
    if expected
        .authenticated_blocks()
        .map_err(DerivedArtifactSealOpenError::Crypto)?
        != reserved.blocks
    {
        return Err(mismatch());
    }
    Ok((expected, current))
}

/// Consumes one fresh committed derived reservation in one exact-key operation.
///
/// The complete C07 descriptor, six-domain residency, Current, AAD, counter,
/// charge and selected key are compared before native key access. All errors
/// consume the permit; its permanently burned nonce is never refunded.
/// Root remains responsible for actual source/policy/root custody.
pub fn seal_derived_artifact<C: RevisionDekContext>(
    permit: CommittedDerivedArtifactSealUse,
    owner: &DerivedArtifactSealBindingV1,
    plaintext: &[u8],
    config: &ValidatedSecretsConfig,
    context: &mut C,
) -> Result<Vec<u8>, DerivedArtifactSealOpenError<C::Error>> {
    let mut window = KeyWindow::new(context, config).map_err(DerivedArtifactSealOpenError::Key)?;
    reserve(&mut window, plaintext.len(), 0)?;
    let descriptor = owner.descriptor();
    let metadata = permit.metadata();
    if descriptor.source() != &metadata.source_revision
        || descriptor.operation_id() != &metadata.operation_id
        || descriptor.materialization_id() != metadata.materialization_id
        || descriptor.representation_id() != metadata.representation_id
        || u64::try_from(plaintext.len()).ok() != Some(permit.plaintext_length())
    {
        return Err(DerivedArtifactSealOpenError::BindingMismatch);
    }
    let (expected, current) = binding(owner, &Reserved::from(&permit), config)?;
    drop(permit);
    window.checkpoint().map_err(|error| {
        DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Context(error))
    })?;
    let key = read_revision_dek_key(&current, &mut window)
        .map_err(|error| DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Dek(error)))?
        .ok_or(DerivedArtifactSealOpenError::Key(
            RevisionSealOpenError::Dek(RevisionDekError::CredentialReadbackInvalid),
        ))?;
    let result = seal_derived_artifact_payload_v1(key.as_bytes(), &expected, plaintext);
    drop(key);
    window.checkpoint().map_err(|error| {
        DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Context(error))
    })?;
    let ciphertext = result.map_err(DerivedArtifactSealOpenError::Crypto)?;
    finish(&mut window, &current).map_err(DerivedArtifactSealOpenError::Key)?;
    Ok(ciphertext)
}

/// Authenticates a derived payload against an exact committed C02 reservation.
///
/// This does not grant source access or publication. The actual Root/C07 adapter
/// must enforce current root, owner, residency, deny and purge conditions before
/// and after this call, under the same original caller context.
pub fn open_derived_artifact<C: RevisionDekContext>(
    reservation: &DerivedArtifactReservationReadback,
    owner: &DerivedArtifactSealBindingV1,
    ciphertext: &[u8],
    config: &ValidatedSecretsConfig,
    context: &mut C,
) -> Result<DerivedArtifactPayloadPlaintextV1, DerivedArtifactSealOpenError<C::Error>> {
    let mut window = KeyWindow::new(context, config).map_err(DerivedArtifactSealOpenError::Key)?;
    let body = usize::try_from(reservation.plaintext_length())
        .map_err(|_| DerivedArtifactSealOpenError::SizeLimit)?;
    reserve(&mut window, body, ciphertext.len())?;
    let descriptor = owner.descriptor();
    if descriptor.source() != reservation.source_revision_ref()
        || descriptor.operation_id() != reservation.operation_id()
        || descriptor.materialization_id() != reservation.materialization_id()
        || descriptor.representation_id() != reservation.representation_id()
        || body.checked_add(REVISION_TAG_BYTES) != Some(ciphertext.len())
    {
        return Err(DerivedArtifactSealOpenError::BindingMismatch);
    }
    let (expected, current) = binding(owner, &Reserved::from(reservation), config)?;
    window.checkpoint().map_err(|error| {
        DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Context(error))
    })?;
    let key = read_revision_dek_key(&current, &mut window)
        .map_err(|error| DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Dek(error)))?
        .ok_or(DerivedArtifactSealOpenError::Key(
            RevisionSealOpenError::Dek(RevisionDekError::CredentialReadbackInvalid),
        ))?;
    let result = open_derived_artifact_payload_v1(key.as_bytes(), &expected, ciphertext);
    drop(key);
    window.checkpoint().map_err(|error| {
        DerivedArtifactSealOpenError::Key(RevisionSealOpenError::Context(error))
    })?;
    let plaintext = result.map_err(DerivedArtifactSealOpenError::Crypto)?;
    finish(&mut window, &current).map_err(DerivedArtifactSealOpenError::Key)?;
    Ok(plaintext)
}
