use std::net::{Shutdown, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use search_provider_protocol::request::RequestCancellation;

use super::model::{Exchange, Outcome};
use super::pipe::write_observed;
use super::spec::ChildLimits;
use super::time::{check_request, deadline, pause, receive, receive_request, remaining};
use super::worker::spawn_worker;
use super::{MAX_PROXY_COMMAND_BYTES, Reply, Terminal};

pub(in super::super) struct ChildIo {
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
    pub(in super::super) fn spawn(
        command: Command,
        limits: ChildLimits,
    ) -> Result<Self, String> {
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
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| "LOOPBACK_DIRECT_CHILD_START_ERROR".to_owned())?;
        let (ready_tx, ready) = mpsc::sync_channel(1);
        // Install the process guard before taking pipes or starting the worker:
        // every subsequent error invokes bounded termination.
        let mut service = Self {
            child,
            commands: None,
            worker: None,
            ready,
            status: None,
            aborted: false,
            cleanup_attempted: false,
            limits,
        };
        let input = service
            .child
            .stdin
            .take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDIN_MISSING".to_owned())?;
        let output = service
            .child
            .stdout
            .take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDOUT_MISSING".to_owned())?;
        let (commands, requests) = mpsc::sync_channel::<Exchange>(1);
        service.commands = Some(commands);
        service.worker = Some(spawn_worker(input, output, ready_tx, requests)?);
        Ok(service)
    }

    pub(in super::super) fn exchange(
        &mut self,
        command: &str,
        socket: &TcpStream,
        terminal: Terminal,
    ) -> Outcome {
        self.exchange_controlled(command, socket, terminal, None)
    }

    /// Anchored before protocol admission, so proof verification and queueing
    /// do not renew the child budget. Round sub-millisecond bounds down.
    pub(in super::super) fn request_budget(&self) -> Result<(Instant, u64), String> {
        let millis = u64::try_from(self.limits.request.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| "LOOPBACK_CHILD_LIMIT_INVALID".to_owned())?;
        Ok((deadline(Duration::from_millis(millis))?, millis))
    }

    /// A control envelope must supply its admitted signal and original deadline.
    /// None is reserved for the existing non-envelope compatibility path.
    pub(in super::super) fn exchange_controlled(
        &mut self,
        command: &str,
        socket: &TcpStream,
        terminal: Terminal,
        control: Option<(Instant, RequestCancellation)>,
    ) -> Outcome {
        self.exchange_observed(command, socket, terminal, control, &mut || Ok(()))
    }

    /// Polls the single connection reader on this owner thread while the pipe
    /// worker runs. A polling failure takes the same fail-stop path as I/O loss.
    pub(in super::super) fn exchange_observed(
        &mut self,
        command: &str,
        socket: &TcpStream,
        terminal: Terminal,
        control: Option<(Instant, RequestCancellation)>,
        poll: &mut dyn FnMut() -> Result<(), String>,
    ) -> Outcome {
        if self.aborted || self.status.is_some() {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if command.len() > MAX_PROXY_COMMAND_BYTES {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let local_deadline = deadline(self.limits.request)?;
        let (deadline, cancellation) = match control {
            Some((original, probe)) => (original.min(local_deadline), Some(probe)),
            None => (local_deadline, None),
        };
        check_request(deadline, cancellation.as_ref())?;
        let original_timeout = socket
            .write_timeout()
            .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
        let (reply, response) = mpsc::sync_channel(1);
        let request = Exchange {
            command: command.to_owned(),
            socket: socket
                .try_clone()
                .map_err(|_| "LOOPBACK_STREAM_CLONE_ERROR".to_owned())?,
            terminal,
            deadline,
            cancellation: cancellation.clone(),
            reply,
        };
        let mut active = ActiveExchange { child: self, socket, completed: false };
        let outcome = (|| {
            poll()?;
            check_request(deadline, cancellation.as_ref())?;
            active.child.commands
                .as_ref()
                .ok_or_else(|| "LOOPBACK_DIRECT_CHANNEL_CLOSED".to_owned())?
                .try_send(request)
                .map_err(|_| "LOOPBACK_DIRECT_PIPE_QUEUE_UNAVAILABLE".to_owned())?;
            let result = receive_request(&response, deadline, cancellation.as_ref(), poll)??;
            if result.reply == Reply::Shutdown {
                // The same request deadline includes child exit and pipe
                // cleanup. STOPPED without process exit is not shutdown.
                let status = active.child.wait_child(deadline, cancellation.as_ref(), poll)?;
                active.child.wait_worker(deadline, cancellation.as_ref(), poll)?;
                if !status.success() {
                    return Err("LOOPBACK_DIRECT_CHILD_EXIT_FAILED".to_owned());
                }
            }
            check_request(deadline, cancellation.as_ref())?;
            if !result.deferred.is_empty() {
                write_observed(socket, &[&result.deferred], deadline, cancellation.as_ref(), poll)?;
            }
            // Cloned TcpStreams share socket options. Do not leak the child
            // budget into the endpoint's next acknowledgement.
            socket
                .set_write_timeout(original_timeout)
                .map_err(|_| "LOOPBACK_SOCKET_CONFIGURATION_ERROR".to_owned())?;
            poll()?;
            check_request(deadline, cancellation.as_ref())?;
            Ok(result.reply)
        })();
        active.completed = outcome.as_ref().is_ok_and(|reply| *reply != Reply::Fatal);
        outcome
    }

    pub(in super::super) fn abort(&mut self) {
        if self.cleanup_attempted {
            return;
        }
        self.cleanup_attempted = true;
        self.aborted = true;
        self.commands.take();
        if self.status.is_none() {
            let _ = self.child.kill();
        }
        if let Ok(deadline) = deadline(self.limits.cleanup) {
            let _ = self.wait_child(deadline, None, &mut || Ok(()));
            let _ = self.wait_worker(deadline, None, &mut || Ok(()));
        }
        // If the OS cannot reap the child or release inherited pipes, do not
        // fabricate clean release. The failed proxy exits with the root still
        // protected by the child or OS lock.
    }

    pub(in super::super) fn finish(&mut self) -> Result<(), String> {
        self.commands.take();
        if self.aborted {
            return Err("LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED".to_owned());
        }
        let end = deadline(self.limits.cleanup)?;
        let result = (|| {
            let status = self.wait_child(end, None, &mut || Ok(()))?;
            self.wait_worker(end, None, &mut || Ok(()))?;
            if status.success() {
                Ok(())
            } else {
                Err("LOOPBACK_DIRECT_CHILD_EXIT_FAILED".to_owned())
            }
        })();
        if result.is_err() {
            self.abort();
        }
        result
    }

    fn wait_child(
        &mut self,
        end: Instant,
        cancellation: Option<&RequestCancellation>,
        poll: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<ExitStatus, String> {
        loop {
            poll()?;
            check_request(end, cancellation)?;
            if let Some(status) = self.status {
                return Ok(status);
            }
            self.status = self
                .child
                .try_wait()
                .map_err(|_| "LOOPBACK_DIRECT_CHILD_WAIT_ERROR".to_owned())?;
            if self.status.is_none() {
                pause(end)?;
            }
        }
    }

    fn wait_worker(
        &mut self,
        end: Instant,
        cancellation: Option<&RequestCancellation>,
        poll: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            poll()?;
            check_request(end, cancellation)?;
            pause(end)?;
        }
        poll()?;
        check_request(end, cancellation)?;
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| "LOOPBACK_DIRECT_PIPE_WORKER_FAILED".to_owned())?;
        }
        Ok(())
    }
}

impl Drop for ChildIo {
    fn drop(&mut self) {
        if self.status.is_none() || self.worker.is_some() {
            self.abort();
        }
    }
}

/// Installed before queue handoff. Cancellation, a late worker return, any
/// failure or unwind closes the client socket and runs bounded process cleanup.
/// Dropping the reply receiver alone would leave a worker able to emit bytes.
struct ActiveExchange<'a> {
    child: &'a mut ChildIo,
    socket: &'a TcpStream,
    completed: bool,
}

impl Drop for ActiveExchange<'_> {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.socket.shutdown(Shutdown::Both);
            self.child.abort();
        }
    }
}
