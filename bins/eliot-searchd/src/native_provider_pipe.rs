//! Hidden Windows pipe mechanics for this package's binary targets.
//!
//! Only the child ABI may use unsafe. Pipe reachability and the private
//! owner/SYSTEM/Administrators ACL confer no pairing, grant or source authority.
use search_provider_protocol::{NATIVE_ENDPOINT_NAME_BYTES, NativeEndpointNameV1};
use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read, Write};
use std::rc::{Rc, Weak};

#[allow(unsafe_code)]
#[path = "native_provider_pipe/child_windows.rs"]
mod platform;

#[cfg(test)]
#[path = "native_provider_pipe/native_transport_tests.rs"]
mod native_transport_tests;

const PIPE_PATH_UNITS: usize = 9 + NATIVE_ENDPOINT_NAME_BYTES + 1;

/// Sole listener retaining one pending instance and at most one accepted stream.
/// No clone, path or native handle escapes. This is not a readiness receipt.
pub struct NativePipeListener {
    name: [u16; PIPE_PATH_UNITS],
    security: platform::PipeSecurity,
    state: RefCell<ListenerState>,
}
struct ListenerState {
    pending: Option<File>,
    active: Weak<()>,
}
impl NativePipeListener {
    /// Claim exactly one canonical endpoint under the original caller context.
    ///
    /// The expected SID digest comes independently from the trusted native-user
    /// owner using the existing digest recipe. The callback polls the original
    /// absolute deadline, cancellation and work custody around native effects;
    /// it must never renew context or substitute a permissive default.
    /// Cleanup cannot be vetoed. No credential access or startup is performed.
    ///
    /// # Errors
    /// Foreign pipe, user mismatch/impersonation, context refusal or native failure.
    /// Errors retain only their kind.
    pub fn create(
        endpoint: NativeEndpointNameV1,
        expected_current_user_digest: [u8; 32],
        check: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Self> {
        check().map_err(redact)?;
        let name = endpoint_path(endpoint)?;
        let security = platform::PipeSecurity::new(expected_current_user_digest, check)?;
        let pending = platform::create_pipe_instance(&name, &security, true, check)?;
        platform::arm(&pending, check)?;
        check().map_err(redact)?;
        Ok(Self {
            name,
            security,
            state: RefCell::new(ListenerState {
                pending: Some(pending),
                active: Weak::new(),
            }),
        })
    }

    /// Poll once under the original context and transfer the original file.
    ///
    /// One active connection blocks further accepts while retaining the pending
    /// instance. Replacement creation/arming precedes transfer, with no namespace
    /// gap. No timers, retries, new tokens or alternate names are created.
    ///
    /// # Errors
    /// `WouldBlock` means no client or the sole connection slot is occupied.
    /// Every other error closes admission and retains only its kind.
    pub fn accept(&self, check: &dyn Fn() -> io::Result<()>) -> io::Result<NativePipeConnection> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| io::ErrorKind::WouldBlock)?;
        let result = self.accept_inner(&mut state, check);
        if result
            .as_ref()
            .is_err_and(|error| error.kind() != io::ErrorKind::WouldBlock)
        {
            drop(state.pending.take());
        }
        result
    }

    fn accept_inner(
        &self,
        state: &mut ListenerState,
        check: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<NativePipeConnection> {
        check().map_err(redact)?;
        let pending = state.pending.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        if state.active.strong_count() != 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        match platform::try_connect(pending, check)? {
            platform::ConnectResult::Pending => return Err(io::ErrorKind::WouldBlock.into()),
            platform::ConnectResult::Disconnected => {
                platform::disconnect(pending, check)?;
                platform::arm(pending, check)?;
                return Err(io::ErrorKind::WouldBlock.into());
            }
            platform::ConnectResult::Connected => {}
        }
        // Original remains live during replacement creation/arming. A single
        // accepted slot plus one pending instance fits the native maximum of two.
        let replacement = platform::create_pipe_instance(&self.name, &self.security, false, check)?;
        platform::arm(&replacement, check)?;
        check().map_err(redact)?;
        let pipe = state
            .pending
            .replace(replacement)
            .ok_or(io::ErrorKind::NotConnected)?;
        let custody = Rc::new(());
        state.active = Rc::downgrade(&custody);
        Ok(NativePipeConnection {
            pipe,
            _custody: custody,
        })
    }

    /// Close admission without waiting or flushing peer buffers.
    pub fn close(&self) {
        drop(self.state.borrow_mut().pending.take());
    }
}

/// Sole accepted synchronous nonblocking byte stream; transport only.
pub struct NativePipeConnection {
    pipe: File,
    _custody: Rc<()>,
}
impl Read for NativePipeConnection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        read_result(self.pipe.read(buffer))
    }
}
impl Write for NativePipeConnection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        write_result(self.pipe.write(buffer), buffer.is_empty())
    }
    fn flush(&mut self) -> io::Result<()> {
        // No userspace buffer. FlushFileBuffers would wait for the peer to read.
        Ok(())
    }
}
impl Drop for NativePipeConnection {
    fn drop(&mut self) {
        platform::disconnect_on_drop(&self.pipe);
    }
}

fn endpoint_path(endpoint: NativeEndpointNameV1) -> io::Result<[u16; PIPE_PATH_UNITS]> {
    let path = format!(r"\\.\pipe\{endpoint}");
    if path.len() + 1 != PIPE_PATH_UNITS || !path.is_ascii() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut units = [0; PIPE_PATH_UNITS];
    for (slot, byte) in units.iter_mut().zip(path.bytes()) {
        if byte == 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        *slot = u16::from(byte);
    }
    // Fixed final element remains NUL and is part of the retained allocation.
    Ok(units)
}
fn redact(error: io::Error) -> io::Error {
    let kind = error.kind();
    drop(error); // Consume and discard all original diagnostic content.
    kind.into()
}

// Same synchronous PIPE_NOWAIT results as the existing client donor.
fn read_result(result: io::Result<usize>) -> io::Result<usize> {
    match result {
        Err(error) if error.raw_os_error() == Some(232) => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if matches!(error.raw_os_error(), Some(109 | 233)) => Ok(0),
        Err(error) => Err(redact(error)),
        value => value,
    }
}
fn write_result(result: io::Result<usize>, empty: bool) -> io::Result<usize> {
    match result {
        Ok(0) if !empty => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if error.raw_os_error() == Some(232) => Err(io::ErrorKind::WouldBlock.into()),
        Err(error) if matches!(error.raw_os_error(), Some(109 | 233)) => {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        Err(error) => Err(redact(error)),
        value => value,
    }
}

#[cfg(test)]
mod fixtures {
    use super::*;
    #[test]
    fn canonical_name_has_one_bounded_nul_terminator() {
        let endpoint =
            NativeEndpointNameV1::parse("eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff")
                .unwrap();
        let name = endpoint_path(endpoint).unwrap();
        assert_eq!(name.last(), Some(&0));
        assert!(name[..name.len() - 1].iter().all(|unit| *unit != 0));
        assert_eq!(
            String::from_utf16(&name[..name.len() - 1]).unwrap(),
            r"\\.\pipe\eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff"
        );
    }
    #[test]
    fn short_io_eof_and_errors_keep_client_semantics() {
        assert_eq!(
            write_result(Ok(0), false).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(write_result(Ok(0), true).unwrap(), 0);
        assert_eq!(write_result(Ok(3), false).unwrap(), 3);
        assert_eq!(read_result(Ok(3)).unwrap(), 3);
        for code in [109, 233] {
            assert_eq!(
                read_result(Err(io::Error::from_raw_os_error(code))).unwrap(),
                0
            );
            assert_eq!(
                write_result(Err(io::Error::from_raw_os_error(code)), false)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::BrokenPipe
            );
        }
        for result in [
            read_result(Err(io::Error::from_raw_os_error(232))),
            write_result(Err(io::Error::from_raw_os_error(232)), false),
        ] {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        }
        for error in [
            read_result(Err(io::Error::from_raw_os_error(5))).unwrap_err(),
            write_result(Err(io::Error::other("private sentinel")), false).unwrap_err(),
        ] {
            assert!(error.raw_os_error().is_none());
            assert!(!format!("{error:?}").contains("sentinel"));
        }
    }
}
