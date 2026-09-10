//! Containment intent: Job Object, filesystem ACL, and loopback binding.
//!
//! This package cannot call the Windows Job Object or ACL APIs from its
//! frozen dependency closure (no `windows-*` crate, `unsafe` forbidden).
//! Containment is therefore an explicit verified precondition, never a
//! silent skip:
//!
//! * daemon composition applies the Job Object and the owner-only data-root
//!   ACL through its qualified platform means and hands over evidence;
//! * [`evaluate_containment`] accepts only coherent evidence and otherwise
//!   fails closed with [`SupervisorError::ContainmentUnavailable`];
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

/// Preconditions the daemon proves before any spawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainmentEvidence {
    method: ContainmentMethod,
    job_object_applied: bool,
    job_object_digest: Option<Blake3Digest32>,
    data_root_acl_applied: bool,
    acl_receipt: Option<ReceiptRef>,
}

impl ContainmentEvidence {
    /// No evidence at all. Any spawn attempt fails closed.
    #[must_use]
    pub const fn missing() -> Self {
        Self {
            method: ContainmentMethod::WindowsJobObjectAndAcl,
            job_object_applied: false,
            job_object_digest: None,
            data_root_acl_applied: false,
            acl_receipt: None,
        }
    }

    /// Product evidence: Job Object plus owner-only ACL, both applied.
    #[must_use]
    pub const fn windows_contained(
        job_object_digest: Blake3Digest32,
        acl_receipt: ReceiptRef,
    ) -> Self {
        Self {
            method: ContainmentMethod::WindowsJobObjectAndAcl,
            job_object_applied: true,
            job_object_digest: Some(job_object_digest),
            data_root_acl_applied: true,
            acl_receipt: Some(acl_receipt),
        }
    }

    /// Explicitly uncontained launch for package-local tests.
    #[must_use]
    pub const fn explicit_uncontained_test_only() -> Self {
        Self {
            method: ContainmentMethod::ExplicitUncontainedTestOnly,
            job_object_applied: false,
            job_object_digest: None,
            data_root_acl_applied: false,
            acl_receipt: None,
        }
    }

    #[must_use]
    pub(crate) const fn method(&self) -> ContainmentMethod {
        self.method
    }

    #[must_use]
    pub(crate) const fn job_object_applied(&self) -> bool {
        self.job_object_applied
    }

    #[must_use]
    pub(crate) const fn job_object_digest(&self) -> Option<Blake3Digest32> {
        self.job_object_digest
    }

    #[must_use]
    pub(crate) const fn data_root_acl_applied(&self) -> bool {
        self.data_root_acl_applied
    }

    pub(crate) const fn acl_receipt(&self) -> Option<&ReceiptRef> {
        self.acl_receipt.as_ref()
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

impl ContainmentReport {
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
        ContainmentMethod::WindowsJobObjectAndAcl => {
            if !evidence.job_object_applied()
                || !evidence.data_root_acl_applied()
                || evidence.job_object_digest().is_none()
                || evidence.acl_receipt().is_none()
            {
                return Err(SupervisorError::ContainmentUnavailable);
            }
            Ok(ContainmentReport {
                contained: true,
                method: ContainmentMethod::WindowsJobObjectAndAcl,
            })
        }
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
    use search_contracts::{Blake3Digest32, ReceiptRef};

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
    fn product_evidence_reports_contained() {
        let evidence = ContainmentEvidence::windows_contained(
            Blake3Digest32::from_bytes([0xA1; 32]),
            ReceiptRef::new("acl-receipt").unwrap(),
        );
        let report = evaluate_containment(&evidence).unwrap();
        assert!(report.contained);
        assert_eq!(report.method, ContainmentMethod::WindowsJobObjectAndAcl);
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
