//! Qualified derived I/O through C07's one hardened immutable-object kernel.
use super::{
    DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1 as Error,
    DerivedArtifactKindV1 as Kind, DerivedArtifactObjectRefV1 as ObjectRef,
    DerivedArtifactPayloadCandidateV1 as Candidate, DerivedArtifactPlaintextV1,
    DerivedArtifactSealBindingV1, DerivedArtifactSetDescriptorV1, DerivedArtifactStorageV1,
    MAX_DERIVED_AAD_BYTES, MAX_DERIVED_DESCRIPTOR_BYTES, MAX_DERIVED_SET_BYTES, codec as wire,
    payload::open_frame,
};
use crate::immutable_object::{publish_cas_object_with_context, read_cas_object_with_context};
use core::fmt;
use std::path::Path;

/// The existing C07 qualified native observation seam, without a second platform adapter.
pub use crate::ContextualLegacyRevisionObjectPlatform as DerivedArtifactPlatformV1;
/// The existing C07 original caller context, without a duplicate trait or meter.
pub use crate::LegacyRevisionObjectContext as DerivedArtifactContextV1;

/// Exact local storage action being admitted by the actual same-root owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactIoOperationV1 {
    /// Admission before any candidate publication or immutable reuse.
    Publish,
    /// Admission before opening an expected stored object.
    Read,
}

/// Required actual owner interface. There is no default or local crypto stub.
/// The parent owns concrete root, C02, Windows and Crypto implementations.
pub trait DerivedArtifactVerificationPortV1<C: DerivedArtifactContextV1> {
    /// Private content-free-rendered owner refusal.
    type Error;
    /// Verify the actual held root, source provenance, policy/purge state, all six
    /// semantic domain bodies and metadata-plaintext permission for this location.
    /// For sealed publication also compare the exact ref/address/stored digest
    /// with the committed C02 late object intent before any filesystem effect.
    /// This check grants no future serving or retention authority.
    fn verify_storage_scope(
        &mut self,
        context: &mut C,
        cas_root: &Path,
        reference: &ObjectRef,
        operation: DerivedArtifactIoOperationV1,
    ) -> Result<(), Self::Error>;
    /// Verify actual producer/profile identity, semantic commitment, exact schema
    /// and provenance. Metadata kinds must contain no source/representation body.
    /// Representation-body bytes must be the actual materializer owner output.
    /// C07 handles its own set schema and reopens its four members instead.
    fn verify_owner_payload(
        &mut self,
        context: &mut C,
        descriptor: &Descriptor,
        encoded_body: &[u8],
    ) -> Result<(), Self::Error>;
    /// Actually authenticate/open this envelope against the committed C02 artifact
    /// reservation and exact selected Current/key/residency. Verify the unchanged
    /// nonce policy and full AAD, not a supplied receipt/digest/boolean. The seal
    /// producer must already have consumed its fresh one-use C02 permit; reads do
    /// not mint another. Return zeroizing plaintext for independent C07 byte checks.
    /// Every nested allocation/native/crypto attempt uses this original context.
    fn open_authenticated(
        &mut self,
        context: &mut C,
        binding: &DerivedArtifactSealBindingV1,
        ciphertext_and_tag: &[u8],
    ) -> Result<DerivedArtifactPlaintextV1, Self::Error>;
}

/// Redacted failure preserving actual owner, original-context and immutable-I/O causes.
pub enum DerivedArtifactIoErrorV1<E, C, V> {
    /// Pure profile/encoded-byte mismatch before a write, or during read validation.
    Profile(Error),
    /// The original caller refused work.
    Context(C),
    /// Actual root/producer/crypto owner verification refused.
    Verification(V),
    /// Preserve owner refusal together with the following original-context refusal.
    AfterVerification {
        /// Actual owner refusal.
        verification: V,
        /// Original caller refusal.
        context: C,
    },
    /// Shared immutable lifecycle failure, including exact possible-effect state.
    Object(crate::ContextualRevisionObjectError<E, C>),
}
impl<E, C, V> DerivedArtifactIoErrorV1<E, C, V> {
    /// True only when the shared kernel reports possible physical effects.
    pub const fn outcome_unknown(&self) -> bool {
        matches!(self, Self::Object(error) if error.outcome_unknown())
    }
}
impl<E, C, V> fmt::Debug for DerivedArtifactIoErrorV1<E, C, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.outcome_unknown() {
            "DERIVED_ARTIFACT_IO_OUTCOME_UNKNOWN"
        } else {
            "DERIVED_ARTIFACT_IO_REFUSED"
        })
    }
}
impl<E, C, V> fmt::Display for DerivedArtifactIoErrorV1<E, C, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
impl<E, C, V> std::error::Error for DerivedArtifactIoErrorV1<E, C, V> {}

/// Actual shared-kernel immutable write/readback receipt. No public constructor,
/// published flag or control reference exists; this is not a control-publication receipt.
pub struct DerivedArtifactWriteReceiptV1<I> {
    reference: ObjectRef,
    native: crate::ContextualRevisionObjectReceipt<I>,
}
impl<I> DerivedArtifactWriteReceiptV1<I> {
    /// Exact verified stored-object equality metadata for the parent reference graph.
    pub const fn object_ref(&self) -> &ObjectRef {
        &self.reference
    }
    /// Actual C07 native identity/length/reuse/deferred-stage observations.
    pub const fn native_receipt(&self) -> &crate::ContextualRevisionObjectReceipt<I> {
        &self.native
    }
}

/// Exact reopened object observation. Source/representation plaintext never escapes
/// this API: the retained bytes are technical metadata or the sealed envelope.
pub struct DerivedArtifactReadReceiptV1<I> {
    reference: ObjectRef,
    native: crate::ContextualRevisionObjectRead<I>,
}
impl<I> DerivedArtifactReadReceiptV1<I> {
    /// Exact stored-object reference independently checked against observed bytes.
    pub const fn object_ref(&self) -> &ObjectRef {
        &self.reference
    }
    /// Original native identity observation, with no root/serving authority.
    pub const fn native_observation(&self) -> &crate::ContextualRevisionObjectRead<I> {
        &self.native
    }
}

type IoResult<P, C, V, T> = Result<
    T,
    DerivedArtifactIoErrorV1<
        <P as crate::LegacyRevisionObjectPlatform>::Error,
        <C as DerivedArtifactContextV1>::Error,
        <V as DerivedArtifactVerificationPortV1<C>>::Error,
    >,
>;

fn charge<P, C, V>(context: &mut C, bytes: usize) -> IoResult<P, C, V, ()>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    context
        .checkpoint()
        .map_err(DerivedArtifactIoErrorV1::Context)?;
    // Prepay bounded profile/parser copies and byte hashing/comparison. Actual
    // I/O, owner parsing, plaintext allocation and native work charge separately.
    context
        .charge(crate::LegacyRevisionObjectWork::Allocate(
            MAX_DERIVED_DESCRIPTOR_BYTES * 16,
        ))
        .map_err(DerivedArtifactIoErrorV1::Context)?;
    context
        .charge(crate::LegacyRevisionObjectWork::Compare(
            MAX_DERIVED_DESCRIPTOR_BYTES * 16,
        ))
        .map_err(DerivedArtifactIoErrorV1::Context)?;
    context
        .charge(crate::LegacyRevisionObjectWork::Compare(bytes))
        .map_err(DerivedArtifactIoErrorV1::Context)
}
fn owner<P, C, V, T>(context: &mut C, result: Result<T, V::Error>) -> IoResult<P, C, V, T>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    match (result, context.checkpoint()) {
        (Err(verification), Err(context)) => Err(DerivedArtifactIoErrorV1::AfterVerification {
            verification,
            context,
        }),
        (Err(error), _) => Err(DerivedArtifactIoErrorV1::Verification(error)),
        (_, Err(error)) => Err(DerivedArtifactIoErrorV1::Context(error)),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Verify the actual owners before effects, then derive the complete CAS path
/// and publish through C07's existing no-clobber/sync/exact-readback kernel.
///
/// The root supplies an already owned CAS root and original attempt name. No
/// stage cleanup, control receipt, source-history append or alias is performed.
pub fn publish_derived_artifact_v1<P, C, V>(
    platform: &P,
    verifier: &mut V,
    context: &mut C,
    cas_root: &Path,
    temporary_name: &str,
    candidate: &Candidate,
    maximum_bytes: usize,
) -> IoResult<P, C, V, DerivedArtifactWriteReceiptV1<P::Identity>>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    verify_stored(
        platform,
        verifier,
        context,
        cas_root,
        candidate.object_ref(),
        candidate.encoded_object(),
        maximum_bytes,
        DerivedArtifactIoOperationV1::Publish,
    )?;
    let reference = candidate.object_ref().clone();
    let address = reference
        .address()
        .map_err(DerivedArtifactIoErrorV1::Profile)?;
    let components = address.components();
    let native = publish_cas_object_with_context(
        platform,
        context,
        cas_root,
        components.each_ref().map(String::as_str),
        &address.stored_digest().to_string(),
        temporary_name,
        candidate.encoded_object(),
        maximum_bytes,
    )
    .map_err(DerivedArtifactIoErrorV1::Object)?;
    Ok(DerivedArtifactWriteReceiptV1 { reference, native })
}

/// Reopen exact CAS bytes and independently verify stored digest, complete
/// descriptor, producer profile, scope and AEAD. No current source path is read.
pub fn read_derived_artifact_v1<P, C, V>(
    platform: &P,
    verifier: &mut V,
    context: &mut C,
    cas_root: &Path,
    expected: &ObjectRef,
    maximum_bytes: usize,
) -> IoResult<P, C, V, DerivedArtifactReadReceiptV1<P::Identity>>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    charge::<P, C, V>(context, MAX_DERIVED_DESCRIPTOR_BYTES)?;
    let result = verifier.verify_storage_scope(
        context,
        cas_root,
        expected,
        DerivedArtifactIoOperationV1::Read,
    );
    owner::<P, C, V, _>(context, result)?;
    let reference = expected.clone();
    let address = reference
        .address()
        .map_err(DerivedArtifactIoErrorV1::Profile)?;
    let components = address.components();
    let native = read_cas_object_with_context(
        platform,
        context,
        cas_root,
        components.each_ref().map(String::as_str),
        &address.stored_digest().to_string(),
        maximum_bytes,
    )
    .map_err(DerivedArtifactIoErrorV1::Object)?;
    verify_stored(
        platform,
        verifier,
        context,
        cas_root,
        expected,
        native.bytes(),
        maximum_bytes,
        DerivedArtifactIoOperationV1::Read,
    )?;
    Ok(DerivedArtifactReadReceiptV1 { reference, native })
}

#[allow(clippy::too_many_arguments)] // Exact root/reference/action stay distinct at this private boundary.
fn verify_stored<P, C, V>(
    platform: &P,
    verifier: &mut V,
    context: &mut C,
    cas_root: &Path,
    reference: &ObjectRef,
    encoded: &[u8],
    maximum_bytes: usize,
    operation: DerivedArtifactIoOperationV1,
) -> IoResult<P, C, V, ()>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    charge::<P, C, V>(context, encoded.len())?;
    if encoded.len() as u64 != reference.stored_bytes
        || wire::hash_bytes(encoded, maximum_bytes).map_err(DerivedArtifactIoErrorV1::Profile)?
            != reference.stored_digest
    {
        return Err(DerivedArtifactIoErrorV1::Profile(Error::Mismatch));
    }
    let result = verifier.verify_storage_scope(context, cas_root, reference, operation);
    owner::<P, C, V, _>(context, result)?;
    match reference.storage() {
        DerivedArtifactStorageV1::MetadataOnly => {
            if !reference.descriptor().kind().is_metadata_only() {
                return Err(DerivedArtifactIoErrorV1::Profile(
                    Error::PlaintextBodyForbidden,
                ));
            }
            reference
                .descriptor()
                .verify_body(encoded, maximum_bytes)
                .map_err(DerivedArtifactIoErrorV1::Profile)?;
            verify_body(
                platform,
                verifier,
                context,
                cas_root,
                reference.descriptor(),
                encoded,
                maximum_bytes,
            )
        }
        DerivedArtifactStorageV1::Aes256GcmEnvelope => {
            context
                .charge(crate::LegacyRevisionObjectWork::Allocate(
                    MAX_DERIVED_AAD_BYTES * 16,
                ))
                .map_err(DerivedArtifactIoErrorV1::Context)?;
            context
                .charge(crate::LegacyRevisionObjectWork::Compare(
                    MAX_DERIVED_AAD_BYTES * 16,
                ))
                .map_err(DerivedArtifactIoErrorV1::Context)?;
            let (binding, ciphertext) =
                open_frame(encoded).map_err(DerivedArtifactIoErrorV1::Profile)?;
            if binding.descriptor() != reference.descriptor() {
                return Err(DerivedArtifactIoErrorV1::Profile(Error::Mismatch));
            }
            let result = verifier.open_authenticated(context, &binding, ciphertext);
            let plaintext = owner::<P, C, V, _>(context, result)?;
            context
                .charge(crate::LegacyRevisionObjectWork::Compare(
                    plaintext.as_bytes().len(),
                ))
                .map_err(DerivedArtifactIoErrorV1::Context)?;
            binding
                .descriptor()
                .verify_body(plaintext.as_bytes(), maximum_bytes)
                .map_err(DerivedArtifactIoErrorV1::Profile)?;
            verify_body(
                platform,
                verifier,
                context,
                cas_root,
                binding.descriptor(),
                plaintext.as_bytes(),
                maximum_bytes,
            )
        }
    }
}
fn verify_body<P, C, V>(
    platform: &P,
    verifier: &mut V,
    context: &mut C,
    cas_root: &Path,
    descriptor: &Descriptor,
    encoded: &[u8],
    maximum_bytes: usize,
) -> IoResult<P, C, V, ()>
where
    C: DerivedArtifactContextV1,
    P: DerivedArtifactPlatformV1<C>,
    V: DerivedArtifactVerificationPortV1<C>,
{
    if descriptor.kind() == Kind::SetManifest {
        let allocation = encoded
            .len()
            .checked_mul(16)
            .filter(|_| encoded.len() <= MAX_DERIVED_SET_BYTES)
            .ok_or(DerivedArtifactIoErrorV1::Profile(Error::BudgetExceeded))?;
        context
            .charge(crate::LegacyRevisionObjectWork::Allocate(allocation))
            .map_err(DerivedArtifactIoErrorV1::Context)?;
        context
            .charge(crate::LegacyRevisionObjectWork::Compare(allocation))
            .map_err(DerivedArtifactIoErrorV1::Context)?;
        let set = DerivedArtifactSetDescriptorV1::decode(encoded)
            .map_err(DerivedArtifactIoErrorV1::Profile)?;
        set.verify_descriptor(descriptor)
            .map_err(DerivedArtifactIoErrorV1::Profile)?;
        // Closed kinds exclude nested sets: depth is one and exactly four exact
        // member reads use the same root, original context and shared kernel.
        for member in set.members() {
            read_derived_artifact_v1(platform, verifier, context, cas_root, member, maximum_bytes)?;
        }
        return context
            .checkpoint()
            .map_err(DerivedArtifactIoErrorV1::Context);
    }
    let result = verifier.verify_owner_payload(context, descriptor, encoded);
    owner::<P, C, V, _>(context, result)
}
