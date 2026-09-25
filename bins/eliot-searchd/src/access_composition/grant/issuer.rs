//! Bounded process-incarnation issuer state.
//!
//! This owner retains only content-free grant templates and receipts. It owns
//! replay/conflict/capacity semantics but delegates operating-system entropy
//! and canonical wall-clock windows to explicit adapters.

use std::collections::BTreeMap;

use search_contracts::{GrantId, OpaqueId, UtcTimestamp};
use search_provider_protocol::MonotonicMillis;

mod validation;

pub use validation::{GrantUseError, GrantValidationClock, VerifiedStandaloneGrant};

use super::{
    GrantIssuerError, StandaloneGrantIssuer, StandaloneGrantMaterial, StandaloneGrantTemplate,
};

const GRANT_ID_BYTES: usize = 16;
const GRANT_NONCE_BYTES: usize = 32;
const NONCE_PREFIX: &str = "grant-nonce-v1:";

/// Operating-system entropy boundary for grant identifiers and nonces.
pub trait GrantEntropySource {
    /// Fills every output byte from one qualified CSPRNG draw.
    fn fill_random(&mut self, output: &mut [u8]) -> Result<(), GrantIssuerError>;
}

/// Exact canonical wall-clock window supplied by a trusted clock adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantTimeWindow {
    issued_at: UtcTimestamp,
    expires_at: UtcTimestamp,
    effective_ttl_ms: u64,
    monotonic_expiry: Option<MonotonicMillis>,
}

impl GrantTimeWindow {
    /// Creates a finite increasing time window.
    pub fn new(
        issued_at: UtcTimestamp,
        expires_at: UtcTimestamp,
        effective_ttl_ms: u64,
    ) -> Result<Self, GrantIssuerError> {
        if effective_ttl_ms == 0 || expires_at <= issued_at {
            return Err(GrantIssuerError::Unavailable);
        }
        Ok(Self {
            issued_at,
            expires_at,
            effective_ttl_ms,
            monotonic_expiry: None,
        })
    }

    /// Binds a native clock observation made before observing the UTC window.
    ///
    /// The expiry is captured once at issuance, never on redemption or replay.
    /// One millisecond is withheld for UTC/millisecond truncation; an adapter
    /// must supply more than one usable millisecond. A wall-only window created
    /// by `new` still supports issuance but cannot pass native grant use checks.
    /// This is trusted clock input, never a timestamp supplied by a client.
    pub fn with_monotonic_origin(
        mut self,
        started: MonotonicMillis,
    ) -> Result<Self, GrantIssuerError> {
        if self.monotonic_expiry.is_some() {
            return Err(GrantIssuerError::Unavailable);
        }
        let budget = self.effective_ttl_ms.checked_sub(1)
            .filter(|millis| *millis > 0).ok_or(GrantIssuerError::Unavailable)?;
        let expiry = started.get().checked_add(budget).ok_or(GrantIssuerError::Unavailable)?;
        self.monotonic_expiry = Some(MonotonicMillis::new(expiry));
        Ok(self)
    }

    /// Canonical issue time.
    #[must_use]
    pub const fn issued_at(&self) -> &UtcTimestamp {
        &self.issued_at
    }

    /// Canonical expiry time.
    #[must_use]
    pub const fn expires_at(&self) -> &UtcTimestamp {
        &self.expires_at
    }

    /// Adapter-observed finite TTL.
    #[must_use]
    pub const fn effective_ttl_ms(&self) -> u64 {
        self.effective_ttl_ms
    }
}

/// Trusted wall-clock boundary for one grant window.
pub trait GrantTimeSource {
    /// Returns an increasing window no wider than `requested_ttl_ms`.
    fn issue_window(
        &mut self,
        requested_ttl_ms: u64,
    ) -> Result<GrantTimeWindow, GrantIssuerError>;
}

/// Finite boot-local issuer with exact operation replay.
///
/// The issuer never evicts retained operation identities: exhaustion fails
/// closed so a later conflicting use cannot be mistaken for a fresh request.
/// A process restart drops every record and changes the template's boot ID,
/// making earlier grants ineligible through the normal live checks.
#[derive(Debug)]
pub struct BoundedStandaloneGrantIssuer<E, T> {
    entropy: E,
    time: T,
    records: BTreeMap<OpaqueId, IssuedGrantRecord>,
    max_records: usize,
    max_entropy_attempts: usize,
}

#[derive(Debug)]
struct IssuedGrantRecord {
    template: StandaloneGrantTemplate,
    material: StandaloneGrantMaterial,
    monotonic_expiry: Option<MonotonicMillis>,
}

impl<E, T> BoundedStandaloneGrantIssuer<E, T> {
    /// Creates an empty finite issuer.
    pub fn new(
        entropy: E,
        time: T,
        max_records: usize,
        max_entropy_attempts: usize,
    ) -> Result<Self, GrantIssuerError> {
        if max_records == 0 || max_entropy_attempts == 0 {
            return Err(GrantIssuerError::CapacityExceeded);
        }
        Ok(Self {
            entropy,
            time,
            records: BTreeMap::new(),
            max_records,
            max_entropy_attempts,
        })
    }

    /// Number of retained exact operation receipts.
    #[must_use]
    pub fn retained_operations(&self) -> usize {
        self.records.len()
    }
}

impl<E, T> StandaloneGrantIssuer for BoundedStandaloneGrantIssuer<E, T>
where
    E: GrantEntropySource,
    T: GrantTimeSource,
{
    fn issue(
        &mut self,
        template: &StandaloneGrantTemplate,
    ) -> Result<StandaloneGrantMaterial, GrantIssuerError> {
        if let Some(retained) = self.records.get(&template.operation_id) {
            return if &retained.template == template {
                Ok(retained.material.clone())
            } else {
                Err(GrantIssuerError::OperationConflict)
            };
        }
        if self.records.len() >= self.max_records {
            return Err(GrantIssuerError::CapacityExceeded);
        }

        let (grant_id, nonce) = self.draw_unique_identity()?;
        let window = self.time.issue_window(template.requested_ttl_ms)?;
        if window.effective_ttl_ms() > template.requested_ttl_ms {
            return Err(GrantIssuerError::Unavailable);
        }

        let material = StandaloneGrantMaterial {
            operation_id: template.operation_id.clone(),
            binding_id: template.binding_id,
            binding_generation: template.binding_generation,
            policy_generation: template.policy_generation,
            grant_id,
            nonce,
            issued_at: window.issued_at().clone(),
            expires_at: window.expires_at().clone(),
            effective_ttl_ms: window.effective_ttl_ms(),
        };
        self.records.insert(
            template.operation_id.clone(),
            IssuedGrantRecord {
                template: template.clone(),
                material: material.clone(),
                monotonic_expiry: window.monotonic_expiry,
            },
        );
        Ok(material)
    }
}

impl<E, T> BoundedStandaloneGrantIssuer<E, T>
where
    E: GrantEntropySource,
{
    fn draw_unique_identity(&mut self) -> Result<(GrantId, OpaqueId), GrantIssuerError> {
        for _ in 0..self.max_entropy_attempts {
            let mut grant_bytes = [0_u8; GRANT_ID_BYTES];
            let mut nonce_bytes = [0_u8; GRANT_NONCE_BYTES];
            self.entropy.fill_random(&mut grant_bytes)?;
            self.entropy.fill_random(&mut nonce_bytes)?;
            if grant_bytes.iter().all(|byte| *byte == 0)
                || nonce_bytes.iter().all(|byte| *byte == 0)
            {
                continue;
            }

            let grant_id = GrantId::from_bytes(grant_bytes);
            let nonce = OpaqueId::new(format!("{NONCE_PREFIX}{}", hex_lower(&nonce_bytes)))
                .map_err(|_| GrantIssuerError::Unavailable)?;
            let collides = self.records.values().any(|retained| {
                retained.material.grant_id == grant_id || retained.material.nonce == nonce
            });
            if !collides {
                return Ok((grant_id, nonce));
            }
        }
        Err(GrantIssuerError::Unavailable)
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
