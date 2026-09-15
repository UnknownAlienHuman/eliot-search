use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

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
