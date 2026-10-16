//! One real Windows local-pipe transport proof. This does not run daemon
//! bootstrap, read or write credentials, perform pairing, or claim readiness.

use super::NativePipeListener;
use search_os_secrets_windows::{
    LegacyRevisionDpapiContext, LegacyRevisionDpapiWork, RevisionDekContext, observe_revision_user,
};
use search_provider_protocol::NativeEndpointNameV1;
use std::{
    cell::Cell,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_ENDPOINT: AtomicU64 = AtomicU64::new(0);

struct ObservationContext {
    deadline: Instant,
}

impl LegacyRevisionDpapiContext for ObservationContext {
    type Error = io::Error;

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        if Instant::now() >= self.deadline {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, _work: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        self.checkpoint()
    }

    fn account_native_clear(&mut self, _actual_bytes: usize) -> Result<(), Self::Error> {
        self.checkpoint()
    }
}

impl RevisionDekContext for ObservationContext {
    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn charge_open(&mut self) -> Result<(), Self::Error> {
        self.checkpoint()
    }
}

fn observed_process_user_digest() -> [u8; 32] {
    let mut context = ObservationContext {
        deadline: Instant::now() + Duration::from_secs(5),
    };
    // Existing token/SID observation only; no credential or secret API is used.
    observe_revision_user(&mut context)
        .expect("observe the actual process token user")
        .user_scope_digest()
}

fn unique_endpoint() -> NativeEndpointNameV1 {
    let sequence = NEXT_ENDPOINT.fetch_add(1, Ordering::Relaxed);
    let uuid = format!(
        "{:08x}-0000-4000-8000-{:012x}",
        std::process::id(),
        sequence
    );
    NativeEndpointNameV1::parse(&format!("eliot-search-v1-{uuid}"))
        .expect("fixture endpoint is canonical")
}

fn open_local_client(endpoint: NativeEndpointNameV1) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!(r"\\.\pipe\{endpoint}"))
}

fn accept_bounded(
    listener: &NativePipeListener,
    check: &dyn Fn() -> io::Result<()>,
) -> io::Result<super::NativePipeConnection> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match listener.accept(check) {
            Ok(connection) => return Ok(connection),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(error),
        }
    }
}

#[test]
fn real_pipe_exchanges_bytes_refuses_second_client_and_cleans_up_on_original_cancel() {
    let expected_user_digest = observed_process_user_digest();
    let endpoint = unique_endpoint();
    let canceled = AtomicBool::new(false);
    let operation_deadline = Instant::now() + Duration::from_secs(10);
    let original_check = || {
        if canceled.load(Ordering::Acquire) {
            Err(io::ErrorKind::Interrupted.into())
        } else if Instant::now() >= operation_deadline {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    };

    let listener = NativePipeListener::create(endpoint, expected_user_digest, &original_check)
        .expect("create one current-user local listener");
    let mut client = open_local_client(endpoint).expect("open the local named-pipe client");
    let mut server = accept_bounded(&listener, &original_check).expect("accept local client");

    let second_client = open_local_client(endpoint).expect("open the second local client instance");
    let second = match listener.accept(&original_check) {
        Err(error) => error,
        Ok(_connection) => panic!("a second concurrent connection was accepted"),
    };
    assert_eq!(second.kind(), io::ErrorKind::WouldBlock);
    drop(second_client);

    let server_bytes = b"native server bytes";
    server.write_all(server_bytes).expect("write server bytes");
    let mut observed_server = [0; 19];
    client
        .read_exact(&mut observed_server)
        .expect("read server bytes at local client");
    assert_eq!(&observed_server, server_bytes);

    let client_bytes = b"native client bytes";
    client.write_all(client_bytes).expect("write client bytes");
    let mut observed_client = [0; 19];
    server
        .read_exact(&mut observed_client)
        .expect("read client bytes at server");
    assert_eq!(&observed_client, client_bytes);

    drop(client);
    drop(server);

    // The third check is ConnectNamedPipe's post-call checkpoint: cancellation
    // there must close the retained pending instance and preserve its cause.
    let checkpoints = Cell::new(0_u8);
    let cancel_after_connect = || {
        let next = checkpoints.get() + 1;
        checkpoints.set(next);
        if next == 3 {
            canceled.store(true, Ordering::Release);
        }
        original_check()
    };
    let canceled_accept = match listener.accept(&cancel_after_connect) {
        Err(error) => error,
        Ok(_connection) => panic!("accept succeeded after its post-call cancellation"),
    };
    assert_eq!(checkpoints.get(), 3, "cancellation follows the native call");
    assert_eq!(canceled_accept.kind(), io::ErrorKind::Interrupted);

    canceled.store(false, Ordering::Release);
    let after_cleanup = match listener.accept(&original_check) {
        Err(error) => error,
        Ok(_connection) => panic!("canceled admission retained a pending instance"),
    };
    assert_eq!(after_cleanup.kind(), io::ErrorKind::NotConnected);
    listener.close();
}
