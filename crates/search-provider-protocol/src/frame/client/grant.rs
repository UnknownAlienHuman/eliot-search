//! Claims are decoded as claims, never minted or accepted as live authority.

use search_contracts::SearchReadGrantClaims;

use crate::error::ProtocolError;
use super::wire::{Decoder, Encoder, Result, Schema, record};

record!(SearchReadGrantClaims {
    grant_id, installation_id, installation_incarnation_id, binding_id,
    principal_opaque_id, client_scope_ref, scope_domain_id, allowed_membership_ids,
    allowed_corpus_or_portfolio_ids, reference_portfolio_revision, allowed_access_partitions,
    allowed_modalities, permitted_recipe_families, maximum_budget_class,
    sensitivity_ceiling, disclosure_ceiling, source_read_permission, exact_scan_permission,
    issued_boot_id, issued_at, expires_at, nonce, revocation_generation,
} => |claims: &SearchReadGrantClaims| claims.validate_shape().map_err(|_| ProtocolError::InvalidBody));
