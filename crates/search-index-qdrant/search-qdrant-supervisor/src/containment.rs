//! Containment intent and post-spawn containment reports.
//!
//! This package cannot call the Windows Job Object or ACL APIs from its
//! frozen dependency closure (no `windows-*` crate, `unsafe` forbidden).
//! A plan cannot claim containment before a process exists. Windows reports
//! are minted only after the daemon's typed spawn port returns the exact child,
//! retained Job custody, and the held-root ACL observation:
//!
//! * daemon composition applies the Job Object and observes the held data-root
//!   ACL through its qualified platform means;
//! * [`evaluate_containment`] accepts only the explicit test-only uncontained
//!   mode; pre-spawn Windows evidence cannot make a launch contained;
//! * test launches state [`ContainmentMethod::ExplicitUncontainedTestOnly`]
//!   explicitly, and every receipt records the resulting status, so an
//!   uncontained run can never be mistaken for contained product execution.
//!
//! [`SupervisorError::ContainmentUnavailable`]: crate::SupervisorError::ContainmentUnavailable

use search_contracts::{Blake3Digest32, ReceiptRef};

use crate::SupervisorError;

/// How the child is contained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainmentMethod {
    /// Daemon-applied Windows Job Object plus owner-only data-root ACL.
    WindowsJobObjectAndAcl,
    /// Explicitly uncontained; package-local tests only. Product gates must
    /// reject receipts carrying this method.
    ExplicitUncontainedTestOnly,
}

/// Requested launch mode. This is not proof that a child is contained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainmentEvidence {
    method: ContainmentMethod,
    spawn_required: bool,
}

impl ContainmentEvidence {
    /// No evidence at all. Any spawn attempt fails closed.
    #[must_use]
    pub const fn missing() -> Self {
        Self {
            method: ContainmentMethod::WindowsJobObjectAndAcl,
            spawn_required: false,
        }
    }

    /// Requests the native Windows Job Object and held data-root ACL spawn path.
    #[must_use]
    pub const fn windows_containment_required() -> Self {
        Self {
            method: ContainmentMethod::WindowsJobObjectAndAcl,
            spawn_required: true,
        }
    }

    /// Explicitly uncontained launch for package-local tests.
    #[must_use]
    pub const fn explicit_uncontained_test_only() -> Self {
        Self {
            method: ContainmentMethod::ExplicitUncontainedTestOnly,
            spawn_required: true,
        }
    }

    #[must_use]
    pub(crate) const fn method(&self) -> ContainmentMethod {
        self.method
    }

    pub(crate) const fn windows_spawn_required(&self) -> bool {
        matches!(self.method, ContainmentMethod::WindowsJobObjectAndAcl) && self.spawn_required
    }
}

/// Verified containment status recorded into every launch receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainmentReport {
    /// True only for fully evidenced Windows containment.
    pub contained: bool,
    /// The method that produced this report.
    pub method: ContainmentMethod,
}

/// Actual, content-free Windows containment observations returned by the
/// daemon's contained-spawn port and retained beside the child custody.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainmentObservation {
    job_object_digest: Blake3Digest32,
    data_root_acl_receipt: ReceiptRef,
}

impl ContainmentObservation {
    /// Job Object policy digest observed by the native spawn owner.
    #[must_use]
    pub const fn job_object_digest(&self) -> Blake3Digest32 {
        self.job_object_digest
    }

    /// Receipt derived from the qualified held data-root ACL observation.
    #[must_use]
    pub const fn data_root_acl_receipt(&self) -> &ReceiptRef {
        &self.data_root_acl_receipt
    }

    pub(crate) const fn from_spawn_port(
        job_object_digest: Blake3Digest32,
        data_root_acl_receipt: ReceiptRef,
    ) -> Self {
        Self {
            job_object_digest,
            data_root_acl_receipt,
        }
    }
}

impl ContainmentReport {
    /// Product report created after the contained-spawn port returns the
    /// actual child, retained Job custody, and held-root ACL observation.
    #[must_use]
    pub(crate) const fn windows_after_spawn(_observation: &ContainmentObservation) -> Self {
        Self {
            contained: true,
            method: ContainmentMethod::WindowsJobObjectAndAcl,
        }
    }

    /// Test-only uncontained report for lifecycle fixtures.
    #[must_use]
    pub const fn for_tests() -> Self {
        Self {
            contained: false,
            method: ContainmentMethod::ExplicitUncontainedTestOnly,
        }
    }
}

/// Evaluates containment evidence without side effects.
pub const fn evaluate_containment(
    evidence: &ContainmentEvidence,
) -> Result<ContainmentReport, SupervisorError> {
    match evidence.method() {
        ContainmentMethod::ExplicitUncontainedTestOnly => Ok(ContainmentReport {
            contained: false,
            method: ContainmentMethod::ExplicitUncontainedTestOnly,
        }),
        ContainmentMethod::WindowsJobObjectAndAcl => Err(SupervisorError::ContainmentUnavailable),
    }
}

/// Loopback-only bind host. Only exact loopback literals are admitted;
/// `localhost` is refused because its resolution is machine-dependent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopbackHost {
    /// `127.0.0.1`.
    V4,
    /// `::1`.
    V6,
}

impl LoopbackHost {
    /// Renders the literal used for sockets and the Qdrant config file.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V4 => "127.0.0.1",
            Self::V6 => "::1",
        }
    }
}

/// Parses and admits only exact loopback literals.
pub fn parse_loopback_host(value: &str) -> Result<LoopbackHost, SupervisorError> {
    match value {
        "127.0.0.1" => Ok(LoopbackHost::V4),
        "::1" => Ok(LoopbackHost::V6),
        _ => Err(SupervisorError::NonLoopbackEndpoint),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ContainmentEvidence, ContainmentMethod, LoopbackHost, evaluate_containment,
        parse_loopback_host,
    };
    use crate::SupervisorError;

    #[test]
    fn missing_evidence_fails_closed() {
        assert_eq!(
            evaluate_containment(&ContainmentEvidence::missing()).unwrap_err(),
            SupervisorError::ContainmentUnavailable
        );
    }

    #[test]
    fn pre_spawn_windows_request_cannot_mint_containment_report() {
        let evidence = ContainmentEvidence::windows_containment_required();
        assert_eq!(
            evaluate_containment(&evidence),
            Err(SupervisorError::ContainmentUnavailable)
        );
    }

    #[test]
    fn missing_windows_containment_is_not_a_spawn_request() {
        assert!(!ContainmentEvidence::missing().windows_spawn_required());
        assert!(ContainmentEvidence::windows_containment_required().windows_spawn_required());
    }

    #[test]
    fn test_only_evidence_reports_uncontained_without_error() {
        let report =
            evaluate_containment(&ContainmentEvidence::explicit_uncontained_test_only()).unwrap();
        assert!(!report.contained);
        assert_eq!(
            report.method,
            ContainmentMethod::ExplicitUncontainedTestOnly
        );
    }

    #[test]
    fn only_exact_loopback_literals_are_admitted() {
        assert_eq!(parse_loopback_host("127.0.0.1").unwrap(), LoopbackHost::V4);
        assert_eq!(parse_loopback_host("::1").unwrap(), LoopbackHost::V6);
        for rejected in [
            "0.0.0.0",
            "::",
            "localhost",
            "127.0.0.2",
            "10.0.0.1",
            "",
            "127.1",
        ] {
            assert_eq!(
                parse_loopback_host(rejected).unwrap_err(),
                SupervisorError::NonLoopbackEndpoint,
                "{rejected:?} must be rejected"
            );
        }
    }
}
