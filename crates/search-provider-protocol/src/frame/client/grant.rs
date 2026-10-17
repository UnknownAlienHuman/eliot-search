//! Claims are decoded as claims, never minted or accepted as live authority.

use search_contracts::{BoundedSet, MAX_SET_ITEMS, SearchReadGrantClaims, evidence::RecipeIdV2};

use crate::error::ProtocolError;
use crate::grant::{
    SearchReadGrantClaimsV2, STANDALONE_EVIDENCE_GRANT_REQUEST_VERSION,
};
use super::wire::{Decoder, Encoder, Result, Schema, record};

record!(SearchReadGrantClaims {
    grant_id, installation_id, installation_incarnation_id, binding_id,
    principal_opaque_id, client_scope_ref, scope_domain_id, allowed_membership_ids,
    allowed_corpus_or_portfolio_ids, reference_portfolio_revision, allowed_access_partitions,
    allowed_modalities, permitted_recipe_families, maximum_budget_class,
    sensitivity_ceiling, disclosure_ceiling, source_read_permission, exact_scan_permission,
    issued_boot_id, issued_at, expires_at, nonce, revocation_generation,
} => |claims: &SearchReadGrantClaims| claims.validate_shape().map_err(|_| ProtocolError::InvalidBody));

impl Schema for SearchReadGrantClaimsV2 {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        self.validate_shape()?;
        output.open(b'{')?;
        let mut first = true;
        output.field(&mut first, "v")?;
        STANDALONE_EVIDENCE_GRANT_REQUEST_VERSION.put(output)?;
        output.field(&mut first, "claims")?;
        self.claims.put(output)?;
        output.field(&mut first, "permitted_evidence_recipes")?;
        self.permitted_evidence_recipes.put(output)?;
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        input.open(b'{')?;
        let mut first = true;
        input.field(&mut first, "v")?;
        let version = <u16 as Schema>::get(input)?;
        if version != STANDALONE_EVIDENCE_GRANT_REQUEST_VERSION {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        input.field(&mut first, "claims")?;
        let claims = SearchReadGrantClaims::get(input)?;
        input.field(&mut first, "permitted_evidence_recipes")?;
        let permitted_evidence_recipes =
            <BoundedSet<RecipeIdV2, MAX_SET_ITEMS> as Schema>::get(input)?;
        input.close(b'}')?;
        let value = Self { claims, permitted_evidence_recipes };
        value.validate_shape()?;
        Ok(value)
    }
}
