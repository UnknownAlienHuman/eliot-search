//! Outcome-fenced ownership of the single DIRECT child process.

use std::env;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use search_provider_protocol::request::RequestGuard;

use super::child_io::{ChildIo, ChildLimits};
use super::exchange::{ExchangeFence, Reply};
use super::{MAX_PROXY_COMMAND_BYTES, Terminal};

pub(super) struct DirectChild {
    io: ChildIo,
    fence: ExchangeFence,
}

impl DirectChild {
    pub(super) fn spawn(root: &Path) -> Result<Self, String> {
        let executable = env::current_exe()
            .map_err(|_| "LOOPBACK_CURRENT_EXE_ERROR".to_owned())?;
        let mut command = Command::new(executable);
        command.arg("--serve-data-root").arg(root);
        Ok(Self {
            io: ChildIo::spawn(command, ChildLimits::DEFAULT)?,
            fence: ExchangeFence::default(),
        })
    }

    /// Forwards one provider-routed child command and reports the consumed
    /// terminal class without interpreting payload semantics.
    pub(super) fn dispatch_provider(
        &mut self,
        child_command: &str,
        terminal: Terminal,
        stream: &TcpStream,
    ) -> Result<crate::provider_composition::ChildReply, String> {
        self.dispatch(child_command, terminal, stream, None, &mut || Ok(()))
    }

    pub(super) fn request_budget(&self) -> Result<(Instant, u64), String> {
        self.io.request_budget()
    }

    /// Uses the actual admitted request, not a separate cancellation flag.
    pub(super) fn dispatch_admitted(
        &mut self,
        child_command: &str,
        terminal: Terminal,
        stream: &TcpStream,
        request: &RequestGuard,
        deadline: Instant,
        poll: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<crate::provider_composition::ChildReply, String> {
        self.dispatch(child_command, terminal, stream, Some((request, deadline)), poll)
    }

    fn dispatch(
        &mut self,
        child_command: &str,
        terminal: Terminal,
        stream: &TcpStream,
        admitted: Option<(&RequestGuard, Instant)>,
        poll: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<crate::provider_composition::ChildReply, String> {
        if self.fence.blocked() {
            self.abort();
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if child_command.is_empty()
            || child_command.len() > MAX_PROXY_COMMAND_BYTES
            || child_command.contains('\n')
            || child_command.contains('\r')
        {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let io = &mut self.io;
        let result = self.fence.run(|| match admitted {
            Some((request, deadline)) => io.exchange_observed(
                child_command,
                stream,
                terminal,
                Some((deadline, request.cancellation())),
                poll,
            ),
            None => io.exchange(child_command, stream, terminal),
        });
        match result {
            Ok(Reply::Complete) => {
                Ok(crate::provider_composition::ChildReply::Complete)
            }
            Ok(Reply::Rejected) => {
                Ok(crate::provider_composition::ChildReply::Rejected)
            }
            Ok(Reply::Shutdown) => {
                Ok(crate::provider_composition::ChildReply::Shutdown)
            }
            Ok(Reply::Fatal) => {
                // ChildIo and ExchangeFence already forbid another dispatch.
                // Let the envelope owner send its one signed unknown outcome
                // before bounded child cleanup consumes the remaining deadline.
                Ok(crate::provider_composition::ChildReply::Fatal)
            }
            Err(_) => {
                self.abort();
                Err("LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED".to_owned())
            }
        }
    }

    pub(super) fn abort(&mut self) {
        self.io.abort();
    }

    pub(super) fn finish(mut self) -> Result<(), String> {
        self.io.finish()
    }
}
