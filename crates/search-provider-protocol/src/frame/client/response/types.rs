//! Supplemental response scalars; shared input scalars keep their existing codec.

use search_contracts::{
    AccessPolicyRevision, AssuranceClass, BoundedBehaviorSignature, BoundedExpression,
    BoundedMap, BoundedNonContentMetadata, BoundedObservation, BoundedText, BoundedTextOrBytes,
    CandidateGapDisposition, CandidateId, CandidateValidationGapReason, CatalogRevision,
    CollectionGenerationId, CollectionRouteRevision, CorpusChangeKind, CountAssurance,
    CoverageDenominatorKind, CoverageGapKind, Epoch, ExactConclusion, ExactItemFailureKind,
    ExactOrEntityBoost, FusionProfileId, LegExecutionState, LegKind, LineageDiversityAction,
    MatchBasis, MembershipRevision, MetadataKey, MetadataScalar, ObservationCursorRevision,
    ObservationFreshnessState, OpaqueAuthorizedFacetValue, OverlayRevision, ProgressPhase,
    ProtocolErrorCode, ProtocolFailureCode, ProtocolRetryability, ProvenanceStepKind,
    PurgeFenceRevision, QuerySnapshotFingerprint, ReceiptRef, Retryability, SearchReasonCodeV1,
    ShadowFenceRevision, SourceId, SourceNamespaceId, SourceOwnerGeneration,
    MAX_BEHAVIOR_SIGNATURE_BYTES, MAX_EXPRESSION_BYTES, MAX_FACET_VALUE_BYTES,
    MAX_METADATA_ENTRIES, MAX_METADATA_KEY_BYTES, MAX_OBSERVATION_BYTES, MAX_OPAQUE_REF_BYTES,
    MAX_PROFILE_ID_BYTES,
};

use crate::error::ProtocolError;
use super::super::wire::{Decoder, Encoder, Result, Schema, record, tagged};

impl Schema for i64 {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.raw(self.to_string().as_bytes()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let text = input.number()?;
        let value: Self = text.parse().map_err(|_| ProtocolError::InvalidBody)?;
        if value.to_string() != text { return Err(ProtocolError::InvalidBody); }
        Ok(value)
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
uuid!(CandidateId, CollectionGenerationId, SourceId, SourceNamespaceId);

macro_rules! revision {
    ($($name:ident),+ $(,)?) => { $(
        impl Schema for $name {
            fn put(&self, output: &mut Encoder) -> Result<()> { self.get().put(output) }
            fn get(input: &mut Decoder<'_>) -> Result<Self> { Ok(Self::new(u64::get(input)?)) }
        }
    )+ };
}
revision!(
    AccessPolicyRevision, CatalogRevision, CollectionRouteRevision, MembershipRevision,
    ObservationCursorRevision, OverlayRevision, PurgeFenceRevision, ShadowFenceRevision,
);

impl Schema for Epoch {
    fn put(&self, output: &mut Encoder) -> Result<()> { self.get().put(output) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(i64::get(input)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

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
digest!(QuerySnapshotFingerprint, SourceOwnerGeneration);

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
text!(BoundedBehaviorSignature, MAX_BEHAVIOR_SIGNATURE_BYTES, new);
text!(BoundedExpression, MAX_EXPRESSION_BYTES, new);
text!(BoundedObservation, MAX_OBSERVATION_BYTES, new);
text!(OpaqueAuthorizedFacetValue, MAX_FACET_VALUE_BYTES, new);
text!(FusionProfileId, MAX_PROFILE_ID_BYTES, new);
text!(ReceiptRef, MAX_OPAQUE_REF_BYTES, new);
text!(MetadataKey, MAX_METADATA_KEY_BYTES, parse);

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
    AssuranceClass, CandidateGapDisposition, CandidateValidationGapReason, CorpusChangeKind,
    CountAssurance, CoverageDenominatorKind, CoverageGapKind, ExactConclusion,
    ExactItemFailureKind, ExactOrEntityBoost, LegExecutionState, LegKind,
    LineageDiversityAction, MatchBasis, ObservationFreshnessState, ProgressPhase,
    ProtocolRetryability, ProvenanceStepKind, Retryability, SearchReasonCodeV1,
);

impl Schema for ProtocolFailureCode {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.text(match self {
            Self::Protocol(code) => code.as_str(),
            Self::Search(code) => code.as_str(),
        })
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let text = input.text(64)?;
        // The P00 public code namespaces are disjoint. Unknown or ambiguous
        // future registrations fail rather than silently choosing a domain.
        match (ProtocolErrorCode::parse(&text), SearchReasonCodeV1::parse(&text)) {
            (Ok(code), Err(_)) => Ok(Self::Protocol(code)),
            (Err(_), Ok(code)) => Ok(Self::Search(code)),
            _ => Err(ProtocolError::InvalidBody),
        }
    }
}

tagged!(MetadataScalar {
    Boolean => "boolean", Unsigned => "unsigned", Signed => "signed",
    DurationMs => "duration_ms", Digest => "digest", ProfileId => "profile_id",
    TemplateId => "template_id",
});

// Only the closed non-content map is serializable here, not arbitrary objects.
impl Schema for BoundedMap<MetadataKey, MetadataScalar, MAX_METADATA_ENTRIES> {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.open(b'{')?;
        let mut first = true;
        for (key, value) in self {
            output.field(&mut first, key.as_str())?;
            value.put(output)?;
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        input.open(b'{')?;
        let mut value = Self::empty();
        let mut previous: Option<MetadataKey> = None;
        if input.peek() != Some(b'}') {
            loop {
                if value.len() >= MAX_METADATA_ENTRIES { return Err(ProtocolError::FrameTooLarge); }
                let key = MetadataKey::get(input)?;
                if previous.as_ref().is_some_and(|old| old >= &key) {
                    return Err(ProtocolError::InvalidBody);
                }
                input.literal(b":")?;
                let scalar = MetadataScalar::get(input)?;
                previous = Some(key.clone());
                value.insert(key, scalar).map_err(|_| ProtocolError::InvalidBody)?;
                if input.peek() != Some(b',') { break; }
                input.literal(b",")?;
            }
        }
        input.close(b'}')?;
        Ok(value)
    }
}
record!(BoundedNonContentMetadata { entries });

impl<const LIMIT: usize> Schema for BoundedText<LIMIT> {
    fn put(&self, output: &mut Encoder) -> Result<()> { output.text(self.as_str()) }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::new(input.text(LIMIT)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl<const TEXT: usize, const BYTES: usize> Schema for BoundedTextOrBytes<TEXT, BYTES> {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        match self {
            Self::Text(value) => { output.tag("text")?; value.put(output)?; }
            Self::Bytes(value) => { output.tag("bytes")?; value.put(output)?; }
        }
        output.close(b'}')
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        let value = match input.tag()?.as_str() {
            "text" => Self::Text(Schema::get(input)?),
            "bytes" => Self::Bytes(Schema::get(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        Ok(value)
    }
}
