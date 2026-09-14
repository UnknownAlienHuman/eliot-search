//! Outcome-fenced ownership of the single DIRECT child process.

use std::env;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;

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
        let result = self
            .fence
            .run(|| io.exchange(child_command, stream, terminal));
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
                self.abort();
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
