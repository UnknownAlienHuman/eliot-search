//! Bounded pipe execution for the one DIRECT child owned by the proxy.
//!
//! Only this guard owns the process. One worker owns its two pipes; a capacity-one
//! queue and a separate one-shot reply per exchange cannot mix old/new responses.
//! The controller can kill the child even while the worker is blocked in pipe I/O.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{Terminal, MAX_PROXY_COMMAND_BYTES};
use super::exchange::{Reply, forward_reply};

const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_LINES: usize = 1_000_000;
const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(10);
type Outcome = Result<Reply, String>;
struct ExchangeOutput { reply: Reply, deferred: Vec<u8> }

#[derive(Clone, Copy)]
pub(super) struct ChildLimits {
    pub(super) startup: Duration,
    pub(super) request: Duration,
    pub(super) cleanup: Duration,
}

impl ChildLimits {
    pub(super) const DEFAULT: Self = Self {
        startup: Duration::from_secs(30),
        request: Duration::from_secs(120),
        cleanup: Duration::from_secs(5),
    };
}

struct Exchange {
    command: String,
    socket: TcpStream,
    terminal: Terminal,
    deadline: Instant,
    reply: SyncSender<Result<ExchangeOutput, String>>,
}

pub(super) struct ChildIo {
    child: Child,
    commands: Option<SyncSender<Exchange>>,
    worker: Option<JoinHandle<()>>,
    ready: Receiver<Result<(), String>>,
    status: Option<ExitStatus>,
    aborted: bool,
    cleanup_attempted: bool,
    limits: ChildLimits,
}

impl ChildIo {
    pub(super) fn spawn(command: Command, limits: ChildLimits) -> Result<Self, String> {
        let deadline = deadline(limits.startup)?;
        let service = Self::launch(command, limits)?;
        receive(&service.ready, deadline)??;
        // Spawn and READY share the same budget; a late message is not success.
        remaining(deadline)?;
        Ok(service)
    }

    fn launch(mut command: Command, limits: ChildLimits) -> Result<Self, String> {
        if limits.startup.is_zero() || limits.request.is_zero() || limits.cleanup.is_zero() {
            return Err("LOOPBACK_CHILD_LIMIT_INVALID".to_owned());
        }
        let child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().map_err(|_| "LOOPBACK_DIRECT_CHILD_START_ERROR".to_owned())?;
        let (ready_tx, ready) = mpsc::sync_channel(1);
        // Install the process guard before taking pipes or starting the worker:
        // every subsequent error invokes bounded termination, including spawn failure.
        let mut service = Self { child, commands: None, worker: None, ready,
            status: None, aborted: false, cleanup_attempted: false, limits };
        let mut input = service.child.stdin.take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDIN_MISSING".to_owned())?;
        let output = service.child.stdout.take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDOUT_MISSING".to_owned())?;
        let (commands, requests) = mpsc::sync_channel::<Exchange>(1);
        service.commands = Some(commands);
        service.worker = Some(thread::Builder::new().name("eliot-direct-pipes".to_owned()).spawn(move || {
            let mut output = BufReader::new(output);
            let ready = read_child_line(&mut output).and_then(|line| match line {
                Some(line) if line.starts_with("{\"event\":\"data_root_ready\",") => Ok(()),
                _ => Err("LOOPBACK_DIRECT_CHILD_NOT_READY".to_owned()),
            });
            let valid = ready.is_ok();
            if ready_tx.send(ready).is_err() || !valid { return; }
            while let Ok(request) = requests.recv() {
                let Exchange { command, socket, terminal, deadline, reply } = request;
                let result = (|| {
                    remaining(deadline)?;
                    input.write_all(command.as_bytes())
                        .and_then(|()| input.write_all(b"\n"))
                        .and_then(|()| input.flush())
                        .map_err(|_| "LOOPBACK_DIRECT_CHILD_WRITE_ERROR".to_owned())?;
                    let mut writer = DeadlineWriter { socket, deadline };
                    let mut deferred = Vec::new();
                    let shutdown = terminal == Terminal::Shutdown;
                    let mut read = || { remaining(deadline)?; read_child_line(&mut output) };
                    // Do not send a clean-stop frame before observing actual
                    // process exit. Retain the child's exact bytes, not a synthetic receipt.
                    let reply = if shutdown {
                        forward_reply(&mut read, &mut deferred, |line| terminal.reached(line),
                            true, MAX_RESPONSE_LINES, 2 * MAX_LINE_BYTES)?
                    } else {
                        forward_reply(&mut read, &mut writer, |line| terminal.reached(line),
                            false, MAX_RESPONSE_LINES, MAX_RESPONSE_BYTES)?
                    };
                    remaining(deadline)?;
                    Ok(ExchangeOutput { reply, deferred })
                })();
                let reusable = result.as_ref().is_ok_and(|out| matches!(out.reply, Reply::Complete | Reply::Rejected));
                if reply.send(result).is_err() || !reusable { break; }
            }
            // Dropping input closes stdin. No shutdown is written after uncertainty.
        }).map_err(|_| "LOOPBACK_DIRECT_PIPE_WORKER_START_ERROR".to_owned())?);
        Ok(service)
    }

    pub(super) fn exchange(&mut self, command: &str, socket: &TcpStream, terminal: Terminal) -> Outcome {
        if self.aborted || self.status.is_some() {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if command.len() > MAX_PROXY_COMMAND_BYTES { return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned()); }
        let deadline = deadline(self.limits.request)?;
        let original_timeout = socket.write_timeout()
            .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
        let (reply, response) = mpsc::sync_channel(1);
        let request = Exchange { command: command.to_owned(),
            socket: socket.try_clone().map_err(|_| "LOOPBACK_STREAM_CLONE_ERROR".to_owned())?,
            terminal, deadline, reply };
        let outcome = (|| {
            self.commands.as_ref().ok_or_else(|| "LOOPBACK_DIRECT_CHANNEL_CLOSED".to_owned())?
                .try_send(request).map_err(|_| "LOOPBACK_DIRECT_PIPE_QUEUE_UNAVAILABLE".to_owned())?;
            let result = receive(&response, deadline)??;
            if result.reply == Reply::Shutdown {
                // The same request deadline includes child exit and pipe cleanup.
                // A child that prints STOPPED then hangs has not shut down.
                let status = self.wait_child(deadline)?;
                self.wait_worker(deadline)?;
                if !status.success() { return Err("LOOPBACK_DIRECT_CHILD_EXIT_FAILED".to_owned()); }
            }
            remaining(deadline)?;
            if !result.deferred.is_empty() {
                let mut writer = DeadlineWriter { socket: socket.try_clone()
                    .map_err(|_| "LOOPBACK_STREAM_CLONE_ERROR".to_owned())?, deadline };
                writer.write_all(&result.deferred).and_then(|()| writer.flush())
                    .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())?;
            }
            // Cloned TcpStreams share socket options. Do not leak the child
            // budget into the endpoint's next acknowledgement or connection step.
            socket.set_write_timeout(original_timeout)
                .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
            Ok(result.reply)
        })();
        if outcome.is_err() || matches!(outcome, Ok(Reply::Fatal)) {
            // Interrupt a slow client write as well as any blocked child pipe.
            let _ = socket.shutdown(Shutdown::Both);
            self.abort();
        }
        outcome
    }

    pub(super) fn abort(&mut self) {
        if self.cleanup_attempted { return; }
        self.cleanup_attempted = true;
        self.aborted = true;
        self.commands.take();
        if self.status.is_none() { let _ = self.child.kill(); }
        if let Ok(deadline) = deadline(self.limits.cleanup) {
            let _ = self.wait_child(deadline);
            let _ = self.wait_worker(deadline);
        }
        // If the OS cannot reap the child or release inherited pipes, do not wait
        // forever or report clean release. The caller exits the failed proxy;
        // the root remains protected by the child/OS lock, not a fabricated receipt.
    }

    pub(super) fn finish(&mut self) -> Result<(), String> {
        self.commands.take();
        if self.aborted { return Err("LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED".to_owned()); }
        let end = deadline(self.limits.cleanup)?;
        let result = (|| {
            let status = self.wait_child(end)?;
            self.wait_worker(end)?;
            if status.success() { Ok(()) } else { Err("LOOPBACK_DIRECT_CHILD_EXIT_FAILED".to_owned()) }
        })();
        if result.is_err() { self.abort(); }
        result
    }

    fn wait_child(&mut self, end: Instant) -> Result<ExitStatus, String> {
        loop {
            if let Some(status) = self.status { return Ok(status); }
            self.status = self.child.try_wait().map_err(|_| "LOOPBACK_DIRECT_CHILD_WAIT_ERROR".to_owned())?;
            if self.status.is_none() { pause(end)?; }
        }
    }

    fn wait_worker(&mut self, end: Instant) -> Result<(), String> {
        while self.worker.as_ref().is_some_and(|worker| !worker.is_finished()) { pause(end)?; }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| "LOOPBACK_DIRECT_PIPE_WORKER_FAILED".to_owned())?;
        }
        Ok(())
    }
}

impl Drop for ChildIo {
    fn drop(&mut self) {
        if self.status.is_none() || self.worker.is_some() { self.abort(); }
    }
}

fn deadline(duration: Duration) -> Result<Instant, String> {
    Instant::now().checked_add(duration).ok_or_else(|| "LOOPBACK_CHILD_LIMIT_INVALID".to_owned())
}
fn remaining(end: Instant) -> Result<Duration, String> {
    let left = end.saturating_duration_since(Instant::now());
    if left.is_zero() { Err("LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED".to_owned()) } else { Ok(left) }
}
fn pause(end: Instant) -> Result<(), String> { thread::sleep(remaining(end)?.min(POLL)); Ok(()) }
fn receive<T>(receiver: &Receiver<T>, end: Instant) -> Result<T, String> {
    receiver.recv_timeout(remaining(end)?).map_err(|error| match error {
        mpsc::RecvTimeoutError::Timeout => "LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED",
        mpsc::RecvTimeoutError::Disconnected => "LOOPBACK_DIRECT_PIPE_WORKER_CLOSED",
    }.to_owned())
}

struct DeadlineWriter { socket: TcpStream, deadline: Instant }
impl Write for DeadlineWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let left = remaining(self.deadline).map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        self.socket.set_write_timeout(Some(left))?;
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        remaining(self.deadline).map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
        self.socket.flush()
    }
}

fn read_child_line(output: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let read = Read::take(&mut *output, (MAX_LINE_BYTES + 1) as u64).read_until(b'\n', &mut bytes)
        .map_err(|_| "LOOPBACK_DIRECT_CHILD_READ_ERROR".to_owned())?;
    if read == 0 { return Ok(None); }
    if bytes.len() > MAX_LINE_BYTES || !bytes.ends_with(b"\n") {
        return Err("LOOPBACK_DIRECT_CHILD_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') { bytes.pop(); }
    String::from_utf8(bytes).map(Some).map_err(|_| "LOOPBACK_DIRECT_CHILD_FRAME_NOT_UTF8".to_owned())
}

