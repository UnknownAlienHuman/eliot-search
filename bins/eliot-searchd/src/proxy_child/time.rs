use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use search_provider_protocol::request::RequestCancellation;

use super::spec::POLL;

pub(super) fn deadline(duration: Duration) -> Result<Instant, String> {
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| "LOOPBACK_CHILD_LIMIT_INVALID".to_owned())
}

pub(super) fn remaining(end: Instant) -> Result<Duration, String> {
    let left = end.saturating_duration_since(Instant::now());
    if left.is_zero() {
        Err("LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED".to_owned())
    } else {
        Ok(left)
    }
}

pub(super) fn pause(end: Instant) -> Result<(), String> {
    thread::sleep(remaining(end)?.min(POLL));
    Ok(())
}

pub(super) fn receive<T>(receiver: &Receiver<T>, end: Instant) -> Result<T, String> {
    receiver
        .recv_timeout(remaining(end)?)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => {
                "LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED"
            }
            mpsc::RecvTimeoutError::Disconnected => {
                "LOOPBACK_DIRECT_PIPE_WORKER_CLOSED"
            }
        }
        .to_owned())
}

/// One request keeps the same deadline and signal through queueing, pipe work
/// and output. This is not proof that a cancelled mutation had no effects.
pub(super) fn check_request(
    end: Instant,
    cancellation: Option<&RequestCancellation>,
) -> Result<(), String> {
    if cancellation.is_some_and(RequestCancellation::is_cancelled) {
        return Err("LOOPBACK_DIRECT_REQUEST_CANCELLED".to_owned());
    }
    remaining(end).map(|_| ())
}

/// The process owner stays responsive while its pipe worker is blocked. Once
/// cancellation is observed the caller closes the socket and terminates the
/// child through the existing bounded cleanup path; it never reuses that pipe.
pub(super) fn receive_request<T>(
    receiver: &Receiver<T>,
    end: Instant,
    cancellation: Option<&RequestCancellation>,
    poll: &mut dyn FnMut() -> Result<(), String>,
) -> Result<T, String> {
    loop {
        check_request(end, cancellation)?;
        poll()?;
        check_request(end, cancellation)?;
        match receiver.recv_timeout(remaining(end)?.min(POLL)) {
            Ok(value) => {
                poll()?;
                check_request(end, cancellation)?;
                return Ok(value);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("LOOPBACK_DIRECT_PIPE_WORKER_CLOSED".to_owned());
            }
        }
    }
}
