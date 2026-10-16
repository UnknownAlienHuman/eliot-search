//! Operation-local backend reconstruction for the first revision-DEK selection.

use search_os_secrets::{
    self as core_secrets, RevisionDekApplicationIntent, RevisionDekCandidate, RevisionDekCurrent,
    RevisionDekSelectionError, RevisionDekSelectionPolicy,
};

use crate::{
    LegacyRevisionDpapiWork, RevisionDekContext, RevisionDekError, RevisionDekReadback,
    read_revision_dek,
};

/// An actual scoped Windows readback retained for one selection operation.
///
/// Construction reads and unwrap-validates the existing candidate. Private fields
/// prevent decoded metadata from recreating this dependency. Retain it through
/// the caller's Native fences and Current publication/readback; it grants no key
/// use, lease or durable application by itself.
pub struct RevisionDekSelectionDependency {
    _readback: RevisionDekReadback,
    policy: RevisionDekSelectionPolicy,
    candidate: RevisionDekCandidate,
}

impl RevisionDekSelectionDependency {
    /// Plans Current metadata only for this dependency's exact policy/candidate.
    ///
    /// This pure step performs no Native operation or control publication. The
    /// caller retains its original context and performs current-custody fences
    /// and the absent-state C02 CAS before reporting applied/selected revisions.
    pub fn prepare_current_record(
        &self,
        intent: &RevisionDekApplicationIntent,
    ) -> Result<RevisionDekCurrent, RevisionDekSelectionError> {
        prepare_matching_record(&self.policy, &self.candidate, intent)
    }
}

/// Reconstructs the fixed backend using exact existing scoped candidate readback.
///
/// Reuses the original context for current-user observation, credential read and
/// nonzero 32-byte unwrap validation. It never generates, writes or deletes a key.
/// The returned owner retains protected readback only; plaintext stays inside the
/// existing read path and is cleared there.
pub fn construct_revision_dek_selection<C: RevisionDekContext>(
    policy: &RevisionDekSelectionPolicy,
    candidate: &RevisionDekCandidate,
    context: &mut C,
) -> Result<RevisionDekSelectionDependency, RevisionDekError<C::Error>> {
    context.checkpoint().map_err(RevisionDekError::Context)?;
    if policy.backend() != core_secrets::REVISION_DEK_SELECTION_BACKEND {
        return Err(RevisionDekError::BindingInvalid);
    }
    let readback = read_revision_dek(candidate.intent().binding(), context)?
        .ok_or(RevisionDekError::CredentialReadbackInvalid)?;
    // Core validation clones the bounded binding twice and hashes the exact
    // observed blob. Reserve that metadata work before invoking its pure owner.
    context
        .charge(LegacyRevisionDpapiWork::Allocate(
            2 * core_secrets::REVISION_DEK_MAX_RECORD_BYTES,
        ))
        .map_err(RevisionDekError::Context)?;
    context
        .charge(LegacyRevisionDpapiWork::Process(
            readback.protected_bytes().len() + 4 * core_secrets::REVISION_DEK_MAX_RECORD_BYTES,
        ))
        .map_err(RevisionDekError::Context)?;
    let actual = RevisionDekCandidate::new(
        candidate.intent(),
        candidate.intent().binding(),
        readback.protected_bytes(),
    )
    .map_err(|_| RevisionDekError::CredentialReadbackInvalid)?;
    if &actual != candidate {
        return Err(RevisionDekError::CredentialReadbackInvalid);
    }
    let dependency = RevisionDekSelectionDependency {
        _readback: readback,
        policy: policy.clone(),
        candidate: actual,
    };
    context.checkpoint().map_err(RevisionDekError::Context)?;
    Ok(dependency)
}

fn prepare_matching_record(
    policy: &RevisionDekSelectionPolicy,
    candidate: &RevisionDekCandidate,
    intent: &RevisionDekApplicationIntent,
) -> Result<RevisionDekCurrent, RevisionDekSelectionError> {
    if intent.policy() != policy || intent.candidate() != candidate {
        return Err(RevisionDekSelectionError::InvalidIntent);
    }
    RevisionDekCurrent::new(intent)
}

#[cfg(test)]
#[path = "revision_selection/tests.rs"]
mod tests;
