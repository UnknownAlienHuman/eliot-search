//! Wire spelling of existing bounded scalar and collection contracts.

use search_contracts::{
    AccessPartitionId, BindingId, Blake3Digest32, BoundedBytes, BoundedCanonicalBytes,
    BoundedDisplayPath, BoundedList, BoundedName, BoundedSet, BoundedSymbolKey, BufferSnapshotId,
    CasePolicy, ComparisonAxis, ContinuationId, CorpusDeltaDimension, CorpusFacetDimension,
    CorpusId, DisclosureCeiling, EntityKind, EvidenceRole, ExactInputDomain, ExactPredicateKind,
    FiniteF64, GitObjectId, GrantId, HandleClass, HandleExpansionKind, HandleId,
    ImportedSnapshotId, InstallationId, InstallationIncarnationId, MessageKind, Modality,
    NonZeroRevision, OpaqueHandleToken, OpaqueId, OpaqueRef, PeerRole, PlanFingerprint, PlanId,
    PortfolioRevision, ProfileId, RecipeIdV1, ReferencePortfolioId, RelationKind,
    RepositoryLineageId, RequestId, ScopeDomainId, SensitivityClass, SourceMembershipId,
    SourceRevisionId, UtcTimestamp, WorkspaceId, WorkspaceViewRevisionId, PositionEncoding,
    MAX_DISPLAY_PATH_BYTES, MAX_HANDLE_TOKEN_BYTES, MAX_NAME_BYTES, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SYMBOL_KEY_BYTES,
};

use super::wire::{Decoder, Encoder, Result, Schema};
use crate::error::ProtocolError;

macro_rules! integer {
    ($($name:ty),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.raw(self.to_string().as_bytes()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                let text = input.number()?;
                let value: Self = text.parse().map_err(|_| ProtocolError::InvalidBody)?;
                if value.to_string() != text { return Err(ProtocolError::InvalidBody); }
                Ok(value)
            }
        }
    )+ };
}
integer!(u8, u16, u32, u64);

impl Schema for bool {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.raw(if *self { b"true" } else { b"false" })
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        if input.peek() == Some(b't') { input.literal(b"true")?; Ok(true) }
        else { input.literal(b"false")?; Ok(false) }
    }
}

impl<T: Schema> Schema for Option<T> {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self { Some(value) => value.put(output), None => output.raw(b"null") }
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        if input.peek() == Some(b'n') { input.literal(b"null")?; Ok(None) }
        else { T::get(input).map(Some) }
    }
}

impl<T: Schema> Schema for Box<T> {
    fn put(&self, output: &mut Encoder) -> Result<()> { self.as_ref().put(output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> { T::get(input).map(Box::new) }
}

fn put_sequence<'a, T: Schema + 'a>(
    items: impl IntoIterator<Item = &'a T>, output: &mut Encoder,
) -> Result<()> {
    output.open(b'[')?;
    let mut first = true;
    for value in items {
        if !first { output.raw(b",")?; }
        first = false;
        value.put(output)?;
    }
    output.close(b']')
}

fn get_sequence<T: Schema, const LIMIT: usize>(input: &mut Decoder<'_>) -> Result<Vec<T>> {
    input.open(b'[')?;
    let mut items = Vec::new();
    if input.peek() != Some(b']') {
        loop {
            if items.len() >= LIMIT { return Err(ProtocolError::FrameTooLarge); }
            items.push(T::get(input)?);
            if input.peek() != Some(b',') { break; }
            input.literal(b",")?;
        }
    }
    input.close(b']')?;
    Ok(items)
}

impl<T: Schema, const LIMIT: usize> Schema for BoundedList<T, LIMIT> {
    fn put(&self, output: &mut Encoder) -> Result<()> { put_sequence(self, output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(get_sequence::<T, LIMIT>(input)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl<T: Schema + Ord, const LIMIT: usize> Schema for BoundedSet<T, LIMIT> {
    fn put(&self, output: &mut Encoder) -> Result<()> { put_sequence(self, output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let items = get_sequence::<T, LIMIT>(input)?;
        // Reject duplicate/reordered input instead of silently sorting it.
        if items.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ProtocolError::InvalidBody);
        }
        Self::from_items(items).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl<const LIMIT: usize> Schema for BoundedBytes<LIMIT> {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.binary(self.as_slice()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(input.binary(LIMIT)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl<const LIMIT: usize> Schema for BoundedCanonicalBytes<LIMIT> {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.binary(self.as_slice()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        // Only the byte container is decoded here. The selected predicate
        // engine must validate its serialized form before planning/execution.
        Self::from_validated(input.binary(LIMIT)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

macro_rules! uuid {
    ($($name:ident),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.text(&self.to_string()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                Self::parse(&input.text(36)?).map_err(|_| ProtocolError::InvalidBody)
            }
        }
    )+ };
}
uuid!(
    AccessPartitionId, BindingId, BufferSnapshotId, ContinuationId, CorpusId, GrantId,
    HandleId, ImportedSnapshotId, InstallationId, InstallationIncarnationId, PlanId,
    ReferencePortfolioId, RepositoryLineageId, RequestId, ScopeDomainId, SourceMembershipId,
    SourceRevisionId, WorkspaceId, WorkspaceViewRevisionId,
);

macro_rules! digest {
    ($($name:ident),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.text(&self.to_string()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                Self::parse_hex(&input.text(64)?).map_err(|_| ProtocolError::InvalidBody)
            }
        }
    )+ };
}
digest!(Blake3Digest32, PlanFingerprint);

macro_rules! text {
    ($name:ident, $limit:expr, $constructor:ident) => {
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.text(self.as_str()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                Self::$constructor(input.text($limit)?).map_err(|_| ProtocolError::InvalidBody)
            }
        }
    };
}
text!(ProfileId, MAX_PROFILE_ID_BYTES, new);
text!(OpaqueId, MAX_OPAQUE_ID_BYTES, new);
text!(OpaqueRef, MAX_OPAQUE_REF_BYTES, new);
text!(BoundedDisplayPath, MAX_DISPLAY_PATH_BYTES, new);
text!(BoundedName, MAX_NAME_BYTES, new);
text!(BoundedSymbolKey, MAX_SYMBOL_KEY_BYTES, new);
text!(GitObjectId, 64, parse);
text!(UtcTimestamp, 64, parse);

macro_rules! registry {
    ($($name:ident),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { output.text(self.as_str()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> {
                Self::parse(&input.text(64)?).map_err(|_| ProtocolError::InvalidBody)
            }
        }
    )+ };
}
registry!(
    CasePolicy, ComparisonAxis, CorpusDeltaDimension, CorpusFacetDimension, DisclosureCeiling,
    EntityKind, EvidenceRole, ExactInputDomain, ExactPredicateKind, HandleClass,
    HandleExpansionKind, MessageKind, Modality, PeerRole, PositionEncoding, RecipeIdV1,
    RelationKind, SensitivityClass,
);

impl Schema for NonZeroRevision {
    fn put(&self, output: &mut Encoder) -> Result<()> { self.get().put(output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(u64::get(input)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl Schema for PortfolioRevision {
    fn put(&self, output: &mut Encoder) -> Result<()> { self.get().put(output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> { Ok(Self::new(u64::get(input)?)) }
}

impl Schema for OpaqueHandleToken {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.binary(self.as_bytes()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(&input.binary(MAX_HANDLE_TOKEN_BYTES)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl Schema for FiniteF64 {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.raw(self.get().to_string().as_bytes()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let text = input.number()?;
        let value: f64 = text.parse().map_err(|_| ProtocolError::InvalidBody)?;
        let finite = Self::new(value).map_err(|_| ProtocolError::InvalidBody)?;
        // Retain finite coordinates (including signed zero), not an integer
        // surrogate. Reject alternate/rounded spellings of the same float.
        if finite.get().to_string() != text { return Err(ProtocolError::InvalidBody); }
        Ok(finite)
    }
}
