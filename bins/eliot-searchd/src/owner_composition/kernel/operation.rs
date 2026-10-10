//! Process-local command context; a request never grants root authority.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText, CanonicalValue,
    DigestInputLimit, OpaqueId, OpaqueRef, RequestId, blake3_canonical,
};
use search_ports::{
    CancellationProbe, IdempotencyClass, MutationIdentity, OperationContext, PackageOpaque,
};
use search_runtime_owner::{OwnerError, OwnerOperation};

pub(super) const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_ARGUMENTS: usize = 256;
pub(super) const MAX_CONTEXT_BYTES: usize = 512 * 1024;
const COMMAND_DEADLINE: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
enum RequestKind {
    Cli,
    ServiceCommand,
}

impl RequestKind {
    const fn domain(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Self::Cli, true) => "eliot/cbor/data-root-cli/windows-native/v1",
            (Self::Cli, false) => "eliot/cbor/data-root-cli/unix-native/v1",
            (Self::ServiceCommand, true) => "eliot/cbor/data-root-command/windows-native/v1",
            (Self::ServiceCommand, false) => "eliot/cbor/data-root-command/unix-native/v1",
        }
    }
}

struct CommandCancellation(Arc<AtomicBool>);

impl fmt::Debug for CommandCancellation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CommandCancellation(<opaque>)")
    }
}

impl PackageOpaque for CommandCancellation {
    fn owner_package(&self) -> &'static str {
        "eliot-searchd"
    }
}

impl CancellationProbe for CommandCancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct RequestState {
    context: OperationContext<CommandCancellation>,
    operation: OwnerOperation,
    deadline: Instant,
    root_locator: PathBuf,
    // Exact validated input survives caller-buffer destruction and is shared
    // with the original deadline/cancellation by every retained child.
    payload: CanonicalValue,
    kind: RequestKind,
}

/// Bounded original request, retained by the native capability and its children.
/// Retaining it shares the same deadline/cancellation, without opening a root.
pub struct DataRootRequest(Arc<RequestState>);

impl fmt::Debug for DataRootRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DataRootRequest(<opaque>)")
    }
}

impl DataRootRequest {
    pub(super) const fn cli_input_domain() -> &'static str {
        RequestKind::Cli.domain()
    }

    pub(crate) fn from_cli(arguments: &[OsString]) -> Result<Self, String> {
        Self::cli_with_deadline(arguments, COMMAND_DEADLINE)
    }

    fn cli_with_deadline(arguments: &[OsString], timeout: Duration) -> Result<Self, String> {
        let started = Instant::now();
        let root = arguments.get(1).ok_or_else(invalid)?;
        let payload = canonical_cli_payload(arguments)?;
        Self::build(
            PathBuf::from(root),
            payload,
            started,
            timeout,
            RequestKind::Cli,
        )
    }

    /// Only the already-admitted service owner supplies this canonical locator.
    pub(crate) fn from_service_command(root: &Path, command: &str) -> Result<Self, String> {
        let started = Instant::now();
        if command.is_empty() || command.len() > MAX_INPUT_BYTES {
            return Err(invalid());
        }
        let payload = CanonicalValue::Array(
            BoundedList::new(vec![
                CanonicalValue::Bytes(
                    BoundedBytes::new(native_argument(root.as_os_str())?).map_err(|_| invalid())?,
                ),
                CanonicalValue::Bytes(
                    BoundedBytes::new(command.as_bytes().to_vec()).map_err(|_| invalid())?,
                ),
            ])
            .map_err(|_| invalid())?,
        );
        Self::build(
            root.to_owned(),
            payload,
            started,
            COMMAND_DEADLINE,
            RequestKind::ServiceCommand,
        )
    }

    fn build(
        root_locator: PathBuf,
        payload: CanonicalValue,
        started: Instant,
        timeout: Duration,
        kind: RequestKind,
    ) -> Result<Self, String> {
        let deadline = started.checked_add(timeout).ok_or_else(invalid)?;
        let millis = u64::try_from(timeout.as_millis()).map_err(|_| invalid())?;
        let name = kind.domain();
        let domain = CanonicalDigestDomain::parse(name).map_err(|_| invalid())?;
        let limit = DigestInputLimit::new(MAX_CONTEXT_BYTES).map_err(|_| invalid())?;
        let digest = blake3_canonical(&domain, &payload, limit).map_err(|_| invalid())?;
        let mut id = [0_u8; 16];
        crate::qualified_entropy::fill_qualified_entropy(&mut id).map_err(str::to_owned)?;
        if id == [0; 16] {
            return Err(invalid());
        }
        let context = OperationContext::new(
            RequestId::from_bytes(id),
            millis,
            CommandCancellation(Arc::new(AtomicBool::new(false))),
            OpaqueRef::new("data-root-command:v1").map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        let operation = OwnerOperation::new(
            MutationIdentity::new(
                OpaqueId::new(RequestId::from_bytes(id).to_string()).map_err(|_| invalid())?,
                IdempotencyClass::SingleAttempt,
            ),
            digest,
        );
        let request = Self(Arc::new(RequestState {
            context,
            operation,
            deadline,
            root_locator,
            payload,
            kind,
        }));
        request.preflight()?;
        Ok(request)
    }

    pub(crate) fn preflight(&self) -> Result<(), String> {
        self.check().map_err(|error| error.code().to_owned())
    }

    pub(crate) fn check(&self) -> Result<(), OwnerError> {
        if self.0.context.cancellation().is_cancelled() {
            return Err(OwnerError::OwnerCancelledBeforeMutation);
        }
        if Instant::now() >= self.0.deadline {
            return Err(OwnerError::OwnerDeadlineInvalid);
        }
        Ok(())
    }

    pub(crate) fn validate_root(&self, root: &Path) -> Result<(), String> {
        self.preflight()?;
        if root != self.0.root_locator {
            return Err(OwnerError::OwnerOperationConflict.code().to_owned());
        }
        Ok(())
    }

    /// Compare initialization's command/root/id with the exact retained input.
    /// This owner-private check grants no root capability and exposes no bytes.
    pub(super) fn validate_cli_inputs(&self, arguments: &[OsString]) -> Result<(), String> {
        self.preflight()?;
        if !matches!(self.0.kind, RequestKind::Cli)
            || canonical_cli_payload(arguments)? != self.0.payload
        {
            return Err(OwnerError::OwnerOperationConflict.code().to_owned());
        }
        self.preflight()
    }

    pub(crate) fn root_locator(&self) -> &Path {
        &self.0.root_locator
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.0.deadline
    }

    pub(crate) fn operation(&self) -> &OwnerOperation {
        &self.0.operation
    }

    pub(crate) fn retain(&self) -> Self {
        Self(Arc::clone(&self.0))
    }

    pub(crate) fn same_context(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Owner-private immutable evidence, not an accessor or recovery authority.
    /// The original digest/domain and qualified id are never reconstructed.
    pub(super) fn retained_input_value(&self) -> Result<CanonicalValue, String> {
        self.preflight()?;
        let value = CanonicalValue::Array(
            BoundedList::new(vec![
                CanonicalValue::Text(
                    CanonicalText::new_non_empty(self.0.kind.domain()).map_err(|_| invalid())?,
                ),
                CanonicalValue::Bytes(
                    BoundedBytes::new(self.0.context.request_id().as_bytes().to_vec())
                        .map_err(|_| invalid())?,
                ),
                CanonicalValue::Bytes(
                    BoundedBytes::new(self.0.operation.request_digest().as_bytes().to_vec())
                        .map_err(|_| invalid())?,
                ),
                self.0.payload.clone(),
            ])
            .map_err(|_| invalid())?,
        );
        self.preflight()?;
        Ok(value)
    }

    pub(crate) fn cancel(&self) {
        self.0
            .context
            .cancellation()
            .0
            .store(true, Ordering::Release);
    }

    /// Every emission checks the original request; failed I/O cancels children.
    pub(crate) const fn output<'a, W: Write>(&'a self, writer: &'a mut W) -> RequestOutput<'a, W> {
        RequestOutput {
            request: self,
            writer,
        }
    }
}

pub struct RequestOutput<'a, W> {
    request: &'a DataRootRequest,
    writer: &'a mut W,
}

impl<W: Write> Write for RequestOutput<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.request.preflight().map_err(io::Error::other)?;
        let result = self.writer.write(bytes);
        if result.is_err() || matches!(result, Ok(0) if !bytes.is_empty()) {
            self.request.cancel();
        }
        // Blocking I/O may return after expiry. Its bytes cannot be recalled,
        // but it must not be acknowledged as a successful complete exchange.
        self.request.preflight().map_err(io::Error::other)?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.request.preflight().map_err(io::Error::other)?;
        let result = self.writer.flush();
        if result.is_err() {
            self.request.cancel();
        }
        self.request.preflight().map_err(io::Error::other)?;
        result
    }
}

fn invalid() -> String {
    "DATA_ROOT_REQUEST_INVALID".to_owned()
}

/// One bounded native-byte array builder serves admission and exact comparison.
fn canonical_cli_payload(arguments: &[OsString]) -> Result<CanonicalValue, String> {
    let root = arguments.get(1).ok_or_else(invalid)?;
    if arguments.len() > MAX_ARGUMENTS || arguments[0].is_empty() || root.is_empty() {
        return Err(invalid());
    }
    let mut total = 0_usize;
    let mut values = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let bytes = native_argument(argument)?;
        total = total.checked_add(bytes.len()).ok_or_else(invalid)?;
        if total > MAX_INPUT_BYTES {
            return Err(invalid());
        }
        values.push(CanonicalValue::Bytes(
            BoundedBytes::new(bytes).map_err(|_| invalid())?,
        ));
    }
    Ok(CanonicalValue::Array(
        BoundedList::new(values).map_err(|_| invalid())?,
    ))
}

fn native_argument(argument: &OsStr) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        if argument.encode_wide().count() > MAX_INPUT_BYTES / 2 {
            return Err(invalid());
        }
        Ok(argument.encode_wide().flat_map(u16::to_le_bytes).collect())
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if argument.as_bytes().len() > MAX_INPUT_BYTES {
            return Err(invalid());
        }
        Ok(argument.as_bytes().to_vec())
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = argument;
        Err(invalid())
    }
}

#[cfg(test)]
mod operation_tests {
    use super::*;
    use std::ffi::OsString;
    use std::io::{self, Write};
    use std::time::Duration;

    /// Windows unpaired surrogate bytes must not be silently lossy-ified.
    #[cfg(windows)]
    fn surrogate_argument() -> OsString {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            // A leading lone surrogate is unpaired; `OsStr::to_str` yields
            // `None`, so any lossy conversion would fabricate path text.
            OsString::from_wide(&[0xD800u16, u16::from(b'x')])
        }
    }

    // ------------------------------------------------------------------
    // Construction, deadline and cancellation
    // ------------------------------------------------------------------

    #[test]
    fn zero_deadline_is_rejected_before_dispatch() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-zero".into()];
        // A zero timeout cannot produce a finite deadline, and
        // `OperationContext::new` refuses a zero relative deadline.
        assert!(DataRootRequest::cli_with_deadline(&arguments, Duration::ZERO).is_err());
        // The default construction path never uses a zero timeout.
        assert!(DataRootRequest::from_cli(&arguments).is_ok());
    }

    #[test]
    fn retained_request_shares_deadline_and_cancellation_with_children() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-shared".into()];
        let parent = DataRootRequest::from_cli(&arguments).expect("request");
        let child = parent.retain();
        // The shared deadline is absolute and identical, never recomputed.
        assert_eq!(parent.deadline(), child.deadline());
        assert_eq!(parent.root_locator(), child.root_locator());
        // Cancelling the parent retires every child through the shared probe.
        parent.cancel();
        assert!(parent.preflight().is_err());
        assert!(child.preflight().is_err());
        // And the reverse: a cancelled child retires the parent.
        let second = DataRootRequest::from_cli(&arguments).expect("request");
        let grandchild = second.retain();
        grandchild.cancel();
        assert!(second.preflight().is_err());
        assert!(grandchild.preflight().is_err());
    }

    #[test]
    fn retained_inputs_survive_caller_changes_and_refuse_another_kind() {
        let mut arguments: Vec<OsString> = vec![
            "--initialize-data-root".into(),
            "private-root".into(),
            "26600000000000000000000000000006".into(),
        ];
        let admitted = arguments.clone();
        let request = DataRootRequest::from_cli(&arguments).unwrap();
        let child = request.retain();
        let digest = request.operation().request_digest();
        arguments[2] = "26600000000000000000000000000007".into();
        assert_eq!(
            request.validate_cli_inputs(&arguments),
            Err("OWNER_OPERATION_CONFLICT".to_owned())
        );
        drop(arguments);
        drop(request);
        assert!(child.validate_cli_inputs(&admitted).is_ok());
        assert_eq!(child.operation().request_digest(), digest);
        assert_eq!(format!("{child:?}"), "DataRootRequest(<opaque>)");
        let service = DataRootRequest::from_service_command(
            std::path::Path::new("private-root"),
            "--initialize-data-root",
        )
        .unwrap();
        assert_eq!(
            service.validate_cli_inputs(&admitted),
            Err("OWNER_OPERATION_CONFLICT".to_owned())
        );
    }

    #[test]
    fn deadline_expiry_is_observed_after_elapsing() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-elapse".into()];
        let request = DataRootRequest::cli_with_deadline(&arguments, Duration::from_millis(25))
            .expect("request");
        assert!(request.preflight().is_ok());
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(
            request.preflight(),
            Err("OWNER_DEADLINE_INVALID".to_owned())
        );
    }

    // ------------------------------------------------------------------
    // Root binding
    // ------------------------------------------------------------------

    #[test]
    fn wrong_requested_root_is_refused_before_mutation() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-first".into()];
        let request = DataRootRequest::from_cli(&arguments).expect("request");
        let foreign = std::path::Path::new("C:\\roots-second");
        assert_eq!(
            request.validate_root(foreign),
            Err("OWNER_OPERATION_CONFLICT".to_owned())
        );
        // The exact admitted root is accepted, and refusal did not cancel it.
        assert!(request.validate_root(request.root_locator()).is_ok());
    }

    // ------------------------------------------------------------------
    // Exact native argument boundaries and order
    // ------------------------------------------------------------------

    #[test]
    fn argument_boundaries_and_order_change_the_request_digest() {
        let base: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-boundary".into()];
        let reference = DataRootRequest::from_cli(&base).expect("request");
        // Splitting one argument into two changes the array, therefore the
        // canonical payload, therefore the digest.
        let split: Vec<OsString> = vec![
            "--health".into(),
            "-data-root".into(),
            "C:\\roots-boundary".into(),
        ];
        let different = DataRootRequest::from_cli(&split).expect("request");
        assert_ne!(
            reference.operation().request_digest(),
            different.operation().request_digest()
        );
        // Reordering the same elements is a different array.
        let reordered: Vec<OsString> =
            vec!["C:\\roots-boundary".into(), "--health-data-root".into()];
        let swapped = DataRootRequest::from_cli(&reordered).expect("request");
        assert_ne!(
            reference.operation().request_digest(),
            swapped.operation().request_digest()
        );
    }

    #[test]
    fn identical_arguments_produce_the_same_digest_but_distinct_ids() {
        let first: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-same".into()];
        let second: Vec<OsString> = first.clone();
        let left = DataRootRequest::from_cli(&first).expect("request");
        let right = DataRootRequest::from_cli(&second).expect("request");
        // Deterministic schema: identical native input yields an identical
        // canonical request digest.
        assert_eq!(
            left.operation().request_digest(),
            right.operation().request_digest()
        );
        // Distinct invocation identities: the request id is qualified entropy,
        // never a digest-derived or clock-derived value.
        assert_ne!(
            left.operation().mutation().operation_id,
            right.operation().mutation().operation_id
        );
    }

    #[test]
    #[cfg(windows)]
    fn windows_unpaired_surrogate_bytes_are_not_lossily_converted() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), surrogate_argument()];
        let request = DataRootRequest::from_cli(&arguments).expect("request");
        // The argument was captured as exact native bytes, so the locator is
        // not a fabricated text spelling and its digest differs from the
        // lossy-replacement spelling.
        let lossy: Vec<OsString> = vec![
            "--health-data-root".into(),
            OsString::from(arguments[1].to_string_lossy().into_owned()),
        ];
        let other = DataRootRequest::from_cli(&lossy).expect("request");
        assert!(request.retain().validate_cli_inputs(&arguments).is_ok());
        assert_eq!(
            request.validate_cli_inputs(&lossy),
            Err("OWNER_OPERATION_CONFLICT".to_owned())
        );
        assert_ne!(
            request.operation().request_digest(),
            other.operation().request_digest()
        );
        assert_ne!(request.root_locator(), other.root_locator());
    }

    // ------------------------------------------------------------------
    // Output and cancellation propagation
    // ------------------------------------------------------------------

    /// A writer that always refuses, simulating a closed or broken sink.
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("refused sink"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("refused sink"))
        }
    }

    #[test]
    fn failed_writer_cancels_the_request() {
        let arguments: Vec<OsString> = vec!["--health-data-root".into(), "C:\\roots-writer".into()];
        let request = DataRootRequest::from_cli(&arguments).expect("request");
        {
            let mut writer = FailingWriter;
            let mut output = request.output(&mut writer);
            assert!(output.write_all(b"payload").is_err());
        }
        // A failed emission cancels the shared probe, so children refuse too.
        assert!(request.preflight().is_err());
        assert!(request.retain().preflight().is_err());
    }

    #[test]
    fn expired_writer_emits_zero_bytes() {
        let arguments: Vec<OsString> = vec![
            "--health-data-root".into(),
            "C:\\roots-expired-writer".into(),
        ];
        let request = DataRootRequest::cli_with_deadline(&arguments, Duration::from_millis(25))
            .expect("request");
        std::thread::sleep(Duration::from_millis(60));
        let mut sink: Vec<u8> = Vec::new();
        {
            let mut output = request.output(&mut sink);
            assert!(output.write_all(b"payload").is_err());
        }
        // No partial frame reached the sink after expiry.
        assert!(sink.is_empty());
    }

    #[test]
    fn cli_and_service_requests_have_disjoint_domains() {
        let root = std::path::Path::new("root");
        let cli = DataRootRequest::from_cli(&["root".into(), "health".into()]).unwrap();
        let service = DataRootRequest::from_service_command(root, "health").unwrap();
        assert_ne!(
            cli.operation().request_digest(),
            service.operation().request_digest()
        );
    }

    #[test]
    fn request_debug_is_content_free() {
        let request =
            DataRootRequest::from_cli(&["private-command".into(), "private-root".into()]).unwrap();
        assert_eq!(format!("{request:?}"), "DataRootRequest(<opaque>)");
    }

    #[test]
    fn cancellation_during_write_cannot_be_acknowledged_as_success() {
        struct CancellingWriter<'a>(&'a DataRootRequest, Vec<u8>);
        impl Write for CancellingWriter<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.1.extend_from_slice(bytes);
                self.0.cancel();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let request = DataRootRequest::from_cli(&["command".into(), "root".into()]).unwrap();
        let mut writer = CancellingWriter(&request, Vec::new());
        assert!(request.output(&mut writer).write_all(b"partial").is_err());
        assert_eq!(writer.1, b"partial");
        assert!(request.preflight().is_err());
    }
}
