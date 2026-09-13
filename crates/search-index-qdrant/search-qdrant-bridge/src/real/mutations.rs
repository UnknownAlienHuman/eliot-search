//! Private live mutation operations. The parent owns the client and ledger;
//! these modules only dispatch bounded requests and verify exact readback.

mod close;
mod delete;
mod readback;
mod upsert;

use super::{BridgeError, OpContext};

/// After dispatch, cancellation is not proof that the write did not commit.
fn check_acknowledgement(completed: bool, context: &OpContext) -> Result<(), BridgeError> {
    if !completed {
        return Err(BridgeError::MutationOutcomeUnknown);
    }
    context.check().map_err(|_| BridgeError::MutationOutcomeUnknown)
}

#[cfg(test)]
mod tests;
