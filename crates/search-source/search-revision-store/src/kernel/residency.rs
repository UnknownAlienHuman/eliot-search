//! Complete typed residency closure and deterministic scope identity.

use search_contracts::{
    AccessDomainId, ConfidentialityDomainId, EncryptionKeyDomainId,
    ErasureDomainId, OpaqueId, RetentionDomainId, ScopeDomainId,
    SearchObjectResidencyKey,
};

use super::error::RevisionStoreError;

/// Complete typed residency closure for one retained revision.
///
/// All six canonical domains are strongly typed. Two closures are equivalent
/// only when every domain is equal; equal content digests alone never imply
/// equivalence. The per-object content digest stays outside this closure (see
/// [`SearchObjectResidencyKey`): it identifies bytes, not the policy domains
/// that authorize their storage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResidencyClosure {
    /// Scope domain owning the source namespace.
    pub scope: ScopeDomainId,
    /// Access domain authorizing reads.
    pub access: AccessDomainId,
    /// Confidentiality domain classifying the bytes.
    pub confidentiality: ConfidentialityDomainId,
    /// Encryption-key domain that wrapped the bytes.
    pub encryption_key: EncryptionKeyDomainId,
    /// Retention domain governing lifetime.
    pub retention: RetentionDomainId,
    /// Erasure domain governing purge.
    pub erasure: ErasureDomainId,
}

impl ResidencyClosure {
    /// Creates one complete residency closure from all six typed domains.
    pub const fn new(
        scope: ScopeDomainId,
        access: AccessDomainId,
        confidentiality: ConfidentialityDomainId,
        encryption_key: EncryptionKeyDomainId,
        retention: RetentionDomainId,
        erasure: ErasureDomainId,
    ) -> Self {
        Self {
            scope,
            access,
            confidentiality,
            encryption_key,
            retention,
            erasure,
        }
    }

    /// Projects the canonical object residency key onto its policy domains.
    ///
    /// The per-object versioned content digest is intentionally dropped: it
    /// identifies bytes, while the closure identifies the domains that must
    /// match before bytes may be physically shared.
    pub const fn from_search_object_key(key: &SearchObjectResidencyKey) -> Self {
        Self {
            scope: key.scope_domain_id,
            access: key.access_domain_id,
            confidentiality: key.confidentiality_domain_id,
            encryption_key: key.encryption_key_domain_id,
            retention: key.retention_domain_id,
            erasure: key.erasure_domain_id,
        }
    }

    /// Derives the deterministic opaque residency scope identity.
    ///
    /// The encoding is versioned (`rs1`) and position-binds every domain as
    /// compact domain UUID hex, so two inequivalent closures never share an
    /// identity. Fresh intents must present exactly this value as their
    /// `residency_key` witness; anything else fails closed unless it travels
    /// through an explicit [`crate::LegacyMigration`].
    pub fn scope_id(&self) -> Result<OpaqueId, RevisionStoreError> {
        let mut text = String::with_capacity(201);
        text.push_str("rs1.");
        push_compact_hex(&mut text, self.scope.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.access.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.confidentiality.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.encryption_key.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.retention.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.erasure.as_bytes());
        OpaqueId::new(text).map_err(|_| RevisionStoreError::AddressInvalid)
    }
}

/// Appends one domain UUID as compact lowercase hexadecimal.
fn push_compact_hex(text: &mut String, bytes: &[u8; 16]) {
    const HEXDIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        text.push(HEXDIGITS[(byte >> 4) as usize] as char);
        text.push(HEXDIGITS[(byte & 0x0F) as usize] as char);
    }
}
