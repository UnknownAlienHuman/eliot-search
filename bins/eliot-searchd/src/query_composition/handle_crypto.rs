//! Root crypto producer for C28 search handles.
//!
//! This adapter supplies only opaque identity bytes, their C00 token digest,
//! and the existing canonical handle time window. It implements no handle
//! store, source protection, authorization or access. Retained-source mint
//! still fails closed unless its caller supplies an actual C28 source-owner
//! guard.

use std::time::{Duration, Instant};

use search_config::{ConfigKeyName, ConfigValue, ValidatedSection};
use search_contracts::{
    CanonicalDigestDomain, DigestInputLimit, HandleId, HandleTokenDigest, OpaqueHandleToken,
    blake3_raw,
};
use search_handles::{HandleCryptoPort, MintMaterial};
use search_ports::CancellationProbe;

use crate::access_composition::{GrantTimeSource, SystemGrantClock};

const HANDLE_ID_BYTES: usize = 16;
const HANDLE_TOKEN_BYTES: usize = 32;
const MINT_ENTROPY_BYTES: usize = HANDLE_ID_BYTES + HANDLE_TOKEN_BYTES;
const HANDLE_TOKEN_DIGEST_DOMAIN: &str = "eliot/raw/handle-token-digest/v1";

/// Closed failure from the root handle crypto adapter.
///
/// Variants carry no token, digest, clock value, or entropy bytes.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum RootHandleCryptoError {
    Cancelled,
    DeadlineExpired,
    EntropyUnavailable,
    ClockUnavailable,
    DigestUnavailable,
    MaterialUnavailable,
    ConfigUnavailable,
    LifetimeExpired,
}

/// Concrete `HandleCryptoPort` bound to the original query work context.
///
/// The caller must borrow its existing qualified `SystemGrantClock` and pass
/// the same request deadline and cancellation probe used by the query. This
/// adapter accepts no caller-supplied wall-clock value and creates no timer.
/// The lifetime comes only from the selected validated `[handles]` section;
/// `HandlePolicy` separately owns record and expansion bounds. The Root caller
/// must select the current section; live restrictive invalidation is separate.
pub(super) struct RootHandleCryptoPort<'a> {
    clock: &'a mut SystemGrantClock,
    deadline: Instant,
    cancellation: &'a (dyn CancellationProbe + Send + Sync),
    ttl_ms: u64,
}

impl<'a> RootHandleCryptoPort<'a> {
    /// Binds the existing clock and original request work boundary.
    pub(super) fn new(
        clock: &'a mut SystemGrantClock,
        handles_config: &ValidatedSection,
        deadline: Instant,
        cancellation: &'a (dyn CancellationProbe + Send + Sync),
    ) -> Result<Self, RootHandleCryptoError> {
        Ok(Self {
            clock,
            deadline,
            cancellation,
            ttl_ms: configured_ttl_ms(handles_config)?,
        })
    }

    fn checkpoint(&self) -> Result<(), RootHandleCryptoError> {
        if self.cancellation.is_cancelled() {
            return Err(RootHandleCryptoError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(RootHandleCryptoError::DeadlineExpired);
        }
        Ok(())
    }
}

impl HandleCryptoPort for RootHandleCryptoPort<'_> {
    type Error = RootHandleCryptoError;

    fn mint_material(&mut self) -> Result<MintMaterial, Self::Error> {
        self.checkpoint()?;

        // This is the original mint window, independent of the work deadline.
        // Observing it before UTC issuance can only shorten the usable lifetime.
        let mint_started = Instant::now();
        let monotonic_deadline = handle_deadline(mint_started, self.ttl_ms)?;
        let window = self
            .clock
            .issue_window(self.ttl_ms)
            .map_err(|_| RootHandleCryptoError::ClockUnavailable)?;
        self.checkpoint()?;

        // One exact OS CSPRNG draw supplies independent HandleId and token
        // bytes. The store remains responsible for collision detection.
        let mut entropy = [0_u8; MINT_ENTROPY_BYTES];
        crate::qualified_entropy::fill_qualified_entropy(&mut entropy)
            .map_err(|_| RootHandleCryptoError::EntropyUnavailable)?;
        self.checkpoint()?;

        let handle_id_bytes: [u8; HANDLE_ID_BYTES] = entropy[..HANDLE_ID_BYTES]
            .try_into()
            .map_err(|_| RootHandleCryptoError::MaterialUnavailable)?;
        let handle_id = HandleId::from_bytes(handle_id_bytes);
        let opaque_token = OpaqueHandleToken::new(&entropy[HANDLE_ID_BYTES..])
            .map_err(|_| RootHandleCryptoError::MaterialUnavailable)?;
        let token_digest = token_digest_for(&opaque_token)?;
        self.checkpoint()?;
        if Instant::now() >= monotonic_deadline {
            return Err(RootHandleCryptoError::LifetimeExpired);
        }

        Ok(MintMaterial {
            handle_id,
            opaque_token,
            token_digest,
            created_at: window.issued_at().clone(),
            expires_at: window.expires_at().clone(),
            monotonic_deadline,
        })
    }

    fn token_digest(&self, token: &OpaqueHandleToken) -> Result<HandleTokenDigest, Self::Error> {
        self.checkpoint()?;
        let digest = token_digest_for(token)?;
        self.checkpoint()?;
        Ok(digest)
    }
}

fn configured_ttl_ms(section: &ValidatedSection) -> Result<u64, RootHandleCryptoError> {
    if section.section_name().as_str() != "handles" || section.owner().as_str() != "search-handles"
    {
        return Err(RootHandleCryptoError::ConfigUnavailable);
    }
    let key = ConfigKeyName::new("ephemeral_ttl_ms", 64)
        .map_err(|_| RootHandleCryptoError::ConfigUnavailable)?;
    let field = section
        .field(&key)
        .ok_or(RootHandleCryptoError::ConfigUnavailable)?;
    let ConfigValue::Integer(value) = field.value else {
        return Err(RootHandleCryptoError::ConfigUnavailable);
    };
    u64::try_from(value)
        .ok()
        .filter(|ttl| *ttl > 0)
        .ok_or(RootHandleCryptoError::ConfigUnavailable)
}

fn handle_deadline(started: Instant, ttl_ms: u64) -> Result<Instant, RootHandleCryptoError> {
    started
        .checked_add(Duration::from_millis(ttl_ms))
        .ok_or(RootHandleCryptoError::ClockUnavailable)
}

fn token_digest_for(token: &OpaqueHandleToken) -> Result<HandleTokenDigest, RootHandleCryptoError> {
    let domain = CanonicalDigestDomain::parse(HANDLE_TOKEN_DIGEST_DOMAIN)
        .map_err(|_| RootHandleCryptoError::DigestUnavailable)?;
    let complete_preimage_limit = domain
        .as_str()
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_add(token.as_bytes().len()))
        .ok_or(RootHandleCryptoError::DigestUnavailable)?;
    let limit = DigestInputLimit::new(complete_preimage_limit)
        .map_err(|_| RootHandleCryptoError::DigestUnavailable)?;
    let digest = blake3_raw(&domain, token.as_bytes(), limit)
        .map_err(|_| RootHandleCryptoError::DigestUnavailable)?;
    Ok(HandleTokenDigest::from_stored_bytes(*digest.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::time::{Duration, Instant};

    use search_config::{
        ConfigDocument, ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath, ConfigLayers,
        ConfigLimits, ConfigOwner, ConfigSectionDescriptor, ConfigSectionName, ConfigSource,
        ConfigSourceKind, ConfigSourceRef, ConfigValue, ConfigValueKind, DocumentValue,
        LayerOperation, ReconfigurationAction, RedactionPolicy, ReloadClass, SecretPolicy,
        SecurityFloor, ValidatedSection, ValueBounds, merge_layers, project_section,
        register_sections,
    };
    use search_contracts::{
        Blake3Digest32, CanonicalDigestDomain, DigestInputLimit, OpaqueHandleToken, ProfileId,
        blake3_raw,
    };

    use super::{HANDLE_TOKEN_DIGEST_DOMAIN, configured_ttl_ms, handle_deadline, token_digest_for};

    // A test-only descriptor fixture for the documented handles contract. The
    // production adapter only consumes a selected validated section; it issues none.
    fn validated_handles(owner: &str, override_ms: Option<i64>) -> ValidatedSection {
        let limits = ConfigLimits::W1;
        let name = ConfigSectionName::new("handles", 64).unwrap();
        let key = ConfigKeyName::new("ephemeral_ttl_ms", 64).unwrap();
        let descriptor = ConfigSectionDescriptor::new(
            name.clone(),
            ConfigOwner::new(owner, 64).unwrap(),
            NonZeroU64::MIN,
            ReloadClass::ApplyLive,
            Blake3Digest32::from_stored_bytes([1; 32]),
            SecretPolicy::ForbidPlaintext,
            [ConfigFieldDescriptor::new(
                key.clone(),
                ConfigValueKind::Integer,
                ConfigValue::Integer(600_000),
                ValueBounds {
                    max_text_bytes: 64,
                    max_list_items: 1,
                    max_list_item_bytes: 64,
                    integer_min: 1,
                    integer_max: i64::MAX,
                },
                [ConfigSourceKind::File],
                false,
                SecurityFloor::None,
                RedactionPolicy::Public,
                [ReconfigurationAction::ApplyLive],
            )
            .unwrap()],
            limits,
        )
        .unwrap();
        let registry = register_sections(1, [descriptor.clone()], limits).unwrap();
        let source = |kind| ConfigSource {
            kind,
            source_ref: ConfigSourceRef::new("handles-ttl-canary", 64).unwrap(),
            source_digest: Blake3Digest32::from_stored_bytes([2; 32]),
        };
        let file = override_ms.map(|ttl| {
            ConfigDocument::from_entries(
                1,
                Some(ProfileId::new("direct").unwrap()),
                source(ConfigSourceKind::File),
                [(
                    ConfigKeyPath::new(name, key),
                    LayerOperation::Set(DocumentValue::Integer(ttl)),
                )],
                limits,
            )
            .unwrap()
        });
        let profile = ProfileId::new("direct").unwrap();
        let merged = merge_layers(
            ConfigLayers {
                defaults: source(ConfigSourceKind::Defaults),
                requested_profile: profile.clone(),
                file,
                environment: None,
                cli: None,
            },
            &registry,
            limits,
        )
        .unwrap();
        ValidatedSection::new(
            project_section(&merged, &descriptor).unwrap(),
            profile,
            limits,
        )
        .unwrap()
    }

    #[test]
    fn configured_ttl_controls_original_mint_window_independently_of_request_deadline() {
        let default = validated_handles("search-handles", None);
        let override_config = validated_handles("search-handles", Some(42_000));
        let default_ms = configured_ttl_ms(&default).unwrap_or_else(|_| panic!("default TTL"));
        let override_ms =
            configured_ttl_ms(&override_config).unwrap_or_else(|_| panic!("file TTL"));
        assert_eq!(default_ms, 600_000);
        assert_eq!(override_ms, 42_000);
        let started = Instant::now();
        let request_deadline = started + Duration::from_millis(1);
        let lifetime = handle_deadline(started, override_ms).unwrap_or_else(|_| panic!("deadline"));
        assert!(lifetime > request_deadline);
        assert_eq!(
            lifetime.duration_since(started),
            Duration::from_millis(42_000)
        );
        assert!(configured_ttl_ms(&validated_handles("foreign-owner", None)).is_err());
    }

    #[test]
    fn token_digest_is_bound_to_the_c00_raw_domain() {
        let token = OpaqueHandleToken::new(&[0xa5; 32]).expect("fixed token");
        let domain = CanonicalDigestDomain::parse(HANDLE_TOKEN_DIGEST_DOMAIN)
            .expect("C00 token digest domain");
        let complete_preimage_limit = domain.as_str().len() + 1 + token.as_bytes().len();
        let limit = DigestInputLimit::new(complete_preimage_limit).expect("bounded digest input");
        let expected = blake3_raw(&domain, token.as_bytes(), limit).expect("C00 raw digest");
        let actual = token_digest_for(&token).unwrap_or_else(|_| panic!("token digest failed"));

        assert_eq!(
            HANDLE_TOKEN_DIGEST_DOMAIN,
            "eliot/raw/handle-token-digest/v1"
        );
        assert!(
            actual.as_bytes() == expected.as_bytes(),
            "C00 token digest binding changed"
        );
    }
}
