//! Indexed-query mode recognition and capability admission.
//!
//! The operation remains on the canonical opaque query frame, but the shared
//! protocol marker makes indexed intent explicit before generic query routing.
//! This module neither constructs Qdrant nor performs retrieval.

use search_provider_protocol::decode_indexed_query;

use super::{
    OpArgument, PROVIDER_INDEXED_QUERY_INVALID,
    PROVIDER_INDEXED_QUERY_UNAVAILABLE, ProviderCapabilities, ProviderDenial,
    ProviderOperation,
};

/// Returns the original indexed query bytes for a marked query operation.
///
/// Ordinary query payloads return `Ok(None)`. A recognized marker with no
/// query is malformed and never falls back to DIRECT query interpretation.
pub fn classify_indexed_query<'a>(
    operation: ProviderOperation,
    argument: &'a OpArgument,
) -> Result<Option<&'a [u8]>, &'static str> {
    if operation != ProviderOperation::Query {
        return Ok(None);
    }
    let OpArgument::Blob(blob) = argument else {
        return Ok(None);
    };
    decode_indexed_query(blob).map_err(|_| PROVIDER_INDEXED_QUERY_INVALID)
}

/// Requires both general query admission and indexed qualification/route
/// readiness. A synthetic inconsistent capability snapshot therefore fails
/// closed even when only one flag is true.
pub fn gate_indexed_query(
    capabilities: &ProviderCapabilities,
) -> Result<(), ProviderDenial> {
    if capabilities.query_available && capabilities.indexed_available {
        Ok(())
    } else {
        Err(ProviderDenial {
            reason: PROVIDER_INDEXED_QUERY_UNAVAILABLE,
            blockers: capabilities.blockers.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use search_provider_protocol::{
        INDEXED_QUERY_MARKER, STRICT_QUERY_PREFIX, encode_indexed_query,
    };

    use super::*;
    use crate::provider_composition::{
        CapabilityEvidence, negotiate_capabilities,
    };

    #[test]
    fn marker_is_distinct_and_capability_gated() {
        let payload = encode_indexed_query(b"needle").expect("valid indexed query");
        let mut blob = STRICT_QUERY_PREFIX.to_vec();
        blob.extend_from_slice(&payload);
        let argument = OpArgument::Blob(blob);
        assert_eq!(
            classify_indexed_query(ProviderOperation::Query, &argument),
            Ok(Some(&b"needle"[..]))
        );

        let ordinary = OpArgument::Blob(b"s:needle".to_vec());
        assert_eq!(
            classify_indexed_query(ProviderOperation::Query, &ordinary),
            Ok(None)
        );

        let mut empty_blob = STRICT_QUERY_PREFIX.to_vec();
        empty_blob.extend_from_slice(INDEXED_QUERY_MARKER);
        assert_eq!(
            classify_indexed_query(
                ProviderOperation::Query,
                &OpArgument::Blob(empty_blob),
            ),
            Err(PROVIDER_INDEXED_QUERY_INVALID)
        );

        let denied = CapabilityEvidence::from_parts(
            true,
            true,
            false,
            vec!["INDEXED_NOT_ACCEPTED"],
        )
        .expect("bounded evidence");
        let denied = negotiate_capabilities(&denied);
        let denial = gate_indexed_query(&denied).expect_err("unqualified indexed query");
        assert_eq!(denial.reason, PROVIDER_INDEXED_QUERY_UNAVAILABLE);
        assert_eq!(denial.blockers, vec!["INDEXED_NOT_ACCEPTED"]);

        let accepted = CapabilityEvidence::from_parts(true, true, true, Vec::new())
            .expect("bounded evidence");
        let accepted = negotiate_capabilities(&accepted);
        assert!(gate_indexed_query(&accepted).is_ok());

        let inconsistent = CapabilityEvidence::from_parts(true, false, true, Vec::new())
            .expect("bounded evidence");
        let inconsistent = negotiate_capabilities(&inconsistent);
        assert!(gate_indexed_query(&inconsistent).is_err());
    }
}
