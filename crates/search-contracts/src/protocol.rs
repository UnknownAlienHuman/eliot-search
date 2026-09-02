use crate::bounds::{
    BoundedBytes, BoundedList, BoundedSet, ContractBoundsV1, MAX_FRAME_BYTES, MAX_LIST_ITEMS,
    MAX_PROTOCOL_IN_FLIGHT, MAX_REASON_CODES, MAX_SET_ITEMS,
};
use crate::canonical::{BoundedNonContentMetadata, OpaqueId, OpaqueRef, UtcTimestamp};
use crate::ids::{
    AccessPolicyRevision, BindingId, Blake3Digest32, CatalogRevision, CollectionRouteRevision,
    ContinuationId, DataRootId, Epoch, GrantId, HandleId, InstallationId,
    InstallationIncarnationId, NonZeroRevision, OpaqueHandleToken, OwnerEpoch, ProfileId,
    RequestId,
};
use crate::lifecycle::{MembershipReadiness, OptionalProviderState};
use crate::query::{NativeAnchor, ObservationFreshness, SearchReadGrantClaims, SourceOwnerFence};
use crate::reasons::{ProtocolErrorCode, SearchReasonCodeV1};
use crate::recipes::{HandleExpansionKind, RecipeIdV1, SearchRecipeRequest};
use crate::results::RecipeResultV1;
use crate::schema::DisclosureCeiling;
use crate::{ContractError, ContractErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProtocolRange {
    pub minimum: ProtocolVersion,
    pub maximum: ProtocolVersion,
}

impl ProtocolRange {
    pub fn new(minimum: ProtocolVersion, maximum: ProtocolVersion) -> Result<Self, ContractError> {
        if minimum > maximum {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "protocol_range",
            ));
        }
        Ok(Self { minimum, maximum })
    }

    #[must_use]
    pub const fn contains(self, version: ProtocolVersion) -> bool {
        version.major >= self.minimum.major
            && version.major <= self.maximum.major
            && (version.major != self.minimum.major || version.minor >= self.minimum.minor)
            && (version.major != self.maximum.major || version.minor <= self.maximum.minor)
    }

    /// Select the highest mutually supported `(major, minor)` version.
    pub fn negotiate(self, peer: Self) -> Result<ProtocolVersion, ProtocolErrorCode> {
        let major_min = self.minimum.major.max(peer.minimum.major);
        let major_max = self.maximum.major.min(peer.maximum.major);
        if major_min > major_max {
            return Err(ProtocolErrorCode::ProtocolVersionMismatch);
        }
        for major in (major_min..=major_max).rev() {
            let minimum_minor = if major == self.minimum.major {
                self.minimum.minor
            } else {
                0
            }
            .max(if major == peer.minimum.major {
                peer.minimum.minor
            } else {
                0
            });
            let maximum_minor = if major == self.maximum.major {
                self.maximum.minor
            } else {
                u16::MAX
            }
            .min(if major == peer.maximum.major {
                peer.maximum.minor
            } else {
                u16::MAX
            });
            if minimum_minor <= maximum_minor {
                return Ok(ProtocolVersion {
                    major,
                    minor: maximum_minor,
                });
            }
        }
        Err(ProtocolErrorCode::ProtocolVersionMismatch)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MessageKind {
    Hello,
    Request,
    Progress,
    Result,
    Error,
    Cancel,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PeerRole {
    Daemon,
    StandaloneCli,
    ClientAdapter,
    Worker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelloBody {
    pub peer_role: PeerRole,
    pub pairing_proof_ref: OpaqueRef,
    pub supported_protocol_range: ProtocolRange,
    pub requested_capability_digest: Option<Blake3Digest32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestBody {
    pub grant: SearchReadGrantClaims,
    pub recipe_request: SearchRecipeRequest,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProgressPhase {
    Accepted,
    Planning,
    Retrieving,
    Validating,
    Projecting,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoundedProgressCounts {
    pub completed_legs: u32,
    pub total_planned_legs: u32,
    pub nominated_candidates: u32,
    pub validated_candidates: u32,
    pub omitted_or_failed_legs: u32,
}

impl BoundedProgressCounts {
    pub fn validate(self) -> Result<(), ContractError> {
        if self
            .completed_legs
            .saturating_add(self.omitted_or_failed_legs)
            > self.total_planned_legs
            || self.validated_candidates > self.nominated_candidates
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "progress_counts",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressBody {
    pub event_sequence: u64,
    pub phase: ProgressPhase,
    pub bounded_counts: BoundedProgressCounts,
    pub degraded_reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultBody {
    pub event_sequence: u64,
    pub result: RecipeResultV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProtocolFailureCode {
    Protocol(ProtocolErrorCode),
    Search(SearchReasonCodeV1),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProtocolRetryability {
    Never,
    SameRequest,
    NewRequestAfterRefresh,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorBody {
    pub code: ProtocolFailureCode,
    pub retryability: ProtocolRetryability,
    pub message_template_id: OpaqueId,
    pub bounded_metadata: BoundedNonContentMetadata,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CancelBody {
    pub target_request_id: RequestId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CancelledBody {
    pub target_request_id: RequestId,
    pub terminal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderBodyV1 {
    Hello(HelloBody),
    Request(RequestBody),
    Progress(ProgressBody),
    Result(ResultBody),
    Error(ErrorBody),
    Cancel(CancelBody),
    Cancelled(CancelledBody),
}

impl ProviderBodyV1 {
    #[must_use]
    pub const fn message_kind(&self) -> MessageKind {
        match self {
            Self::Hello(_) => MessageKind::Hello,
            Self::Request(_) => MessageKind::Request,
            Self::Progress(_) => MessageKind::Progress,
            Self::Result(_) => MessageKind::Result,
            Self::Error(_) => MessageKind::Error,
            Self::Cancel(_) => MessageKind::Cancel,
            Self::Cancelled(_) => MessageKind::Cancelled,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEnvelope {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub binding_id: BindingId,
    pub connection_sequence: u64,
    pub request_id: RequestId,
    pub message_kind: MessageKind,
    pub relative_deadline_ms: Option<u64>,
    pub body: ProviderBodyV1,
}

impl ProviderEnvelope {
    /// Validate the tagged body and require the envelope version to be in the negotiated range.
    pub fn validate_version_and_limits(
        &self,
        supported: ProtocolRange,
    ) -> Result<(), ContractError> {
        self.validate()?;
        if !supported.contains(self.protocol_version()) {
            return Err(ContractError::unsupported_version(
                "provider_envelope.protocol_version",
            ));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.message_kind != self.body.message_kind() {
            return Err(ContractError::new(
                ContractErrorKind::InvalidTaggedVariant,
                "provider_envelope.body",
            ));
        }
        if let ProviderBodyV1::Progress(progress) = &self.body {
            progress.bounded_counts.validate()?;
        }
        Ok(())
    }

    #[must_use]
    pub const fn protocol_version(&self) -> ProtocolVersion {
        ProtocolVersion {
            major: self.protocol_major,
            minor: self.protocol_minor,
        }
    }
}

/// A validated UTF-8 provider JSON payload. The transport framing is separate
/// from record canonicalization and never performs fragmented assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonFramePayload(BoundedBytes<MAX_FRAME_BYTES>);

impl JsonFramePayload {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolErrorCode> {
        if bytes.len().saturating_add(4) > MAX_FRAME_BYTES {
            return Err(ProtocolErrorCode::FrameTooLarge);
        }
        core::str::from_utf8(&bytes).map_err(|_| ProtocolErrorCode::InvalidEnvelope)?;
        BoundedBytes::new(bytes)
            .map(Self)
            .map_err(|_| ProtocolErrorCode::FrameTooLarge)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Provider transport: `u32` little-endian payload length followed by UTF-8 JSON.
pub fn encode_json_frame(
    payload: &JsonFramePayload,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolErrorCode> {
    let length =
        u32::try_from(payload.as_slice().len()).map_err(|_| ProtocolErrorCode::FrameTooLarge)?;
    let total = payload.as_slice().len().saturating_add(4);
    if total > MAX_FRAME_BYTES {
        return Err(ProtocolErrorCode::FrameTooLarge);
    }
    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(payload.as_slice());
    BoundedBytes::new(frame).map_err(|_| ProtocolErrorCode::FrameTooLarge)
}

pub fn decode_json_frame(frame: &[u8]) -> Result<JsonFramePayload, ProtocolErrorCode> {
    if frame.len() < 4 || frame.len() > MAX_FRAME_BYTES {
        return Err(if frame.len() > MAX_FRAME_BYTES {
            ProtocolErrorCode::FrameTooLarge
        } else {
            ProtocolErrorCode::InvalidEnvelope
        });
    }
    let declared = u32::from_le_bytes(
        frame[..4]
            .try_into()
            .map_err(|_| ProtocolErrorCode::InvalidEnvelope)?,
    );
    let declared = usize::try_from(declared).map_err(|_| ProtocolErrorCode::FrameTooLarge)?;
    if declared != frame.len().saturating_sub(4) {
        return Err(ProtocolErrorCode::InvalidEnvelope);
    }
    JsonFramePayload::new(frame[4..].to_vec())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchProviderCapabilityDescriptor {
    pub provider_protocol_version: ProtocolVersion,
    pub installation_id: InstallationId,
    pub installation_incarnation_id: InstallationIncarnationId,
    pub data_root_identity: OpaqueId,
    pub owner_epoch: OwnerEpoch,
    pub source_owner_generations: BoundedList<SourceOwnerFence, MAX_LIST_ITEMS>,
    pub supported_recipes: BoundedSet<RecipeIdV1, MAX_SET_ITEMS>,
    pub available_profiles: BoundedSet<ProfileId, MAX_SET_ITEMS>,
    pub optional_provider_states: BoundedList<OptionalProviderState, MAX_LIST_ITEMS>,
    pub visible_epoch: Option<Epoch>,
    pub collection_route_revision: CollectionRouteRevision,
    pub access_policy_generation: AccessPolicyRevision,
    pub source_inventory_revision: CatalogRevision,
    pub observation_freshness: ObservationFreshness,
    pub readiness_by_membership: BoundedList<MembershipReadiness, MAX_LIST_ITEMS>,
    pub degraded_reason_codes: BoundedSet<SearchReasonCodeV1, MAX_REASON_CODES>,
}

impl SearchProviderCapabilityDescriptor {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.observation_freshness.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HandleClass {
    Ephemeral,
    DurableSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSourceHandle {
    pub handle_id: HandleId,
    pub handle_revision: NonZeroRevision,
    pub handle_class: HandleClass,
    pub expires_at: Option<UtcTimestamp>,
    pub opaque_token: OpaqueHandleToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationHandle {
    pub continuation_id: ContinuationId,
    pub expires_at: UtcTimestamp,
    pub opaque_token: OpaqueHandleToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleExpansionRequest {
    pub expansion: HandleExpansionKind,
    pub max_bytes: u64,
    pub requested_anchor: Option<NativeAnchor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandlePermitId {
    Source(HandleId),
    Continuation(ContinuationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandlePermit {
    pub handle_id: HandlePermitId,
    pub binding_id: BindingId,
    pub authorization_generation_digest: Blake3Digest32,
    pub disclosure_ceiling: DisclosureCeiling,
    pub maximum_bytes: u64,
    pub expires_at: UtcTimestamp,
    pub permit_digest: Blake3Digest32,
}

/// Baseline protocol limits exported as a single closed value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolLimits {
    pub contract_bounds: ContractBoundsV1,
    pub frame_bytes: u64,
    pub in_flight_requests: u32,
    pub compression_enabled: bool,
    pub fragmented_assembly_enabled: bool,
}

impl ProtocolLimits {
    pub fn p00() -> Result<Self, ContractError> {
        Ok(Self {
            contract_bounds: ContractBoundsV1::p00()?,
            frame_bytes: u64::try_from(MAX_FRAME_BYTES).unwrap_or(u64::MAX),
            in_flight_requests: u32::try_from(MAX_PROTOCOL_IN_FLIGHT).unwrap_or(u32::MAX),
            compression_enabled: false,
            fragmented_assembly_enabled: false,
        })
    }
}

crate::impl_wire_enum!(MessageKind {
    Hello => "hello",
    Request => "request",
    Progress => "progress",
    Result => "result",
    Error => "error",
    Cancel => "cancel",
    Cancelled => "cancelled",
});
crate::impl_wire_enum!(PeerRole {
    Daemon => "daemon",
    StandaloneCli => "standalone_cli",
    ClientAdapter => "client_adapter",
    Worker => "worker",
});
crate::impl_wire_enum!(ProgressPhase {
    Accepted => "accepted",
    Planning => "planning",
    Retrieving => "retrieving",
    Validating => "validating",
    Projecting => "projecting",
});
crate::impl_wire_enum!(ProtocolRetryability {
    Never => "never",
    SameRequest => "same_request",
    NewRequestAfterRefresh => "new_request_after_refresh",
});
crate::impl_wire_enum!(HandleClass {
    Ephemeral => "ephemeral",
    DurableSource => "durable_source",
});

const _: Option<(DataRootId, GrantId)> = None;
