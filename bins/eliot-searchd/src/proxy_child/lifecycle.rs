use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

use super::model::{Exchange, Outcome};
use super::pipe::DeadlineWriter;
use super::spec::ChildLimits;
use super::time::{deadline, pause, receive, remaining};
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
        if self.aborted || self.status.is_some() {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if command.len() > MAX_PROXY_COMMAND_BYTES {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let deadline = deadline(self.limits.request)?;
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
            reply,
        };
        let outcome = (|| {
            self.commands
                .as_ref()
                .ok_or_else(|| "LOOPBACK_DIRECT_CHANNEL_CLOSED".to_owned())?
                .try_send(request)
                .map_err(|_| "LOOPBACK_DIRECT_PIPE_QUEUE_UNAVAILABLE".to_owned())?;
            let result = receive(&response, deadline)??;
            if result.reply == Reply::Shutdown {
                // The same request deadline includes child exit and pipe
                // cleanup. STOPPED without process exit is not shutdown.
                let status = self.wait_child(deadline)?;
                self.wait_worker(deadline)?;
                if !status.success() {
                    return Err("LOOPBACK_DIRECT_CHILD_EXIT_FAILED".to_owned());
                }
            }
            remaining(deadline)?;
            if !result.deferred.is_empty() {
                let mut writer = DeadlineWriter {
                    socket: socket
                        .try_clone()
                        .map_err(|_| "LOOPBACK_STREAM_CLONE_ERROR".to_owned())?,
                    deadline,
                };
                writer
                    .write_all(&result.deferred)
                    .and_then(|()| writer.flush())
                    .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())?;
            }
            // Cloned TcpStreams share socket options. Do not leak the child
            // budget into the endpoint's next acknowledgement.
            socket
                .set_write_timeout(original_timeout)
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
            let _ = self.wait_child(deadline);
            let _ = self.wait_worker(deadline);
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
            let status = self.wait_child(end)?;
            self.wait_worker(end)?;
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

    fn wait_child(&mut self, end: Instant) -> Result<ExitStatus, String> {
        loop {
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

    fn wait_worker(&mut self, end: Instant) -> Result<(), String> {
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            pause(end)?;
        }
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
