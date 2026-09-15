use super::{ChildIo, ChildLimits, Terminal};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

fn short_limits() -> ChildLimits {
    ChildLimits {
        startup: Duration::from_millis(300),
        request: Duration::from_millis(400),
        cleanup: Duration::from_millis(500),
    }
}

fn hanging_silent_command() -> Command {
    #[cfg(windows)]
    {
        // `timeout` refuses piped stdin; `ping` stays alive without stdout.
        let mut command = Command::new("cmd");
        command.args(["/C", "ping -n 6 127.0.0.1 > NUL"]);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5"]);
        command
    }
}

fn ready_then_hanging_command(_guard: &mut Option<TempStub>) -> Command {
    #[cfg(windows)]
    {
        // Write an exact batch file instead of relying on nested cmd quoting.
        let stub = TempStub::new_batch(
            "@echo {\"event\":\"data_root_ready\",\"ready\":true}\r\n@ping -n 6 127.0.0.1 > NUL\r\n",
        );
        let mut command = Command::new("cmd");
        command.args(["/C", &stub.path]);
        *_guard = Some(stub);
        command
    }
    #[cfg(not(windows))]
    {
        let _ = _guard;
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "printf '%s\\n' '{\"event\":\"data_root_ready\",\"ready\":true}'; sleep 5",
        ]);
        command
    }
}

struct TempStub {
    path: String,
}

impl TempStub {
    #[cfg(windows)]
    fn new_batch(contents: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "eliot-ready-stub-{}-{}.cmd",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).unwrap();
        Self {
            path: path.to_string_lossy().into_owned(),
        }
    }
}

impl Drop for TempStub {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn loopback_socket() -> TcpStream {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let client = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
    let (server, _) = listener.accept().unwrap();
    drop(client);
    server
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
}

#[test]
fn child_budgets_are_finite_distinct_declared_bounds() {
    assert_eq!(ChildLimits::DEFAULT.startup, Duration::from_secs(30));
    assert_eq!(ChildLimits::DEFAULT.request, Duration::from_secs(120));
    assert_eq!(ChildLimits::DEFAULT.cleanup, Duration::from_secs(5));
    for limit in [
        ChildLimits::DEFAULT.startup,
        ChildLimits::DEFAULT.request,
        ChildLimits::DEFAULT.cleanup,
    ] {
        assert!(!limit.is_zero());
    }
    assert!(ChildLimits::DEFAULT.request > ChildLimits::DEFAULT.startup);
    assert!(ChildLimits::DEFAULT.startup > ChildLimits::DEFAULT.cleanup);
}

#[test]
fn hang_before_ready_times_out_and_reaps_within_declared_bounds() {
    let start = Instant::now();
    let result = ChildIo::spawn(hanging_silent_command(), short_limits());
    let elapsed = start.elapsed();
    assert!(result.is_err(), "a child that never prints READY must not spawn");
    let error = result.err().unwrap();
    assert!(
        error == "LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED"
            || error == "LOOPBACK_DIRECT_PIPE_WORKER_CLOSED",
        "{error}"
    );
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
}

#[test]
fn hang_mid_response_times_out_without_indefinite_pipe_write() {
    let mut guard = None;
    let mut child = ChildIo::spawn(ready_then_hanging_command(&mut guard), short_limits())
        .expect("ready-then-hanging stub must spawn");
    let socket = loopback_socket();
    let start = Instant::now();
    let result = child.exchange("health", &socket, Terminal::Single);
    let elapsed = start.elapsed();
    assert!(result.is_err(), "a hanging mid-response must time out");
    assert_eq!(
        result.unwrap_err(),
        "LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED"
    );
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    let socket = loopback_socket();
    assert_eq!(
        child.exchange("health", &socket, Terminal::Single),
        Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned())
    );
}

#[test]
fn hang_at_shutdown_is_reaped_within_request_plus_cleanup() {
    let mut guard = None;
    let mut child = ChildIo::spawn(ready_then_hanging_command(&mut guard), short_limits())
        .expect("ready-then-hanging stub must spawn");
    let socket = loopback_socket();
    let start = Instant::now();
    let result = child.exchange("shutdown", &socket, Terminal::Shutdown);
    let elapsed = start.elapsed();
    assert!(result.is_err(), "a hanging shutdown must time out");
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
}
