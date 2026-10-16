//! Private Windows translation only; never selected by startup automatically.
#![allow(dead_code)]

use super::super::StandaloneProcessOwner;
use super::local::{
    StandaloneLocalBindError, StandaloneLocalListener, StandaloneLocalOwner, check_setup,
    check_setup_io,
};
use crate::provider_composition::LocalByteStream;
use eliot_searchd::native_provider_pipe::{NativePipeConnection, NativePipeListener};
use search_contracts::ProtocolRange;
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{MonotonicMillis, ProtocolLimits};
use std::{io, time::Duration};

impl StandaloneLocalListener for NativePipeListener {
    type Stream = NativePipeConnection;

    fn accept(&self, check: &dyn Fn() -> io::Result<()>) -> io::Result<Self::Stream> {
        Self::accept(self, check)
    }

    fn close(&self) {
        Self::close(self);
    }
}

impl LocalByteStream for NativePipeConnection {
    fn configure_nonblocking(&self) -> io::Result<()> {
        // Creation fixes byte read mode and PIPE_NOWAIT; no handle escapes that
        // could mutate the mode, so there is no additional native configuration.
        Ok(())
    }
}

impl StandaloneLocalOwner<NativePipeListener> {
    /// Parent must supply trusted user evidence and its original absolute setup
    /// deadline. This factory claims transport only, not readiness/source access.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_windows<C: CancellationProbe>(
        process: StandaloneProcessOwner,
        accept_quantum: Duration,
        local_protocols: ProtocolRange,
        limits: ProtocolLimits,
        expected_current_user_digest: [u8; 32],
        context: &OperationContext<C>,
        absolute_deadline: MonotonicMillis,
    ) -> Result<Self, StandaloneLocalBindError> {
        check_setup(context, absolute_deadline)?;
        let owner = Self::bind(process, accept_quantum, local_protocols, limits, |name| {
            NativePipeListener::create(name, expected_current_user_digest, &|| {
                check_setup_io(context, absolute_deadline)
            })
        })?;
        check_setup(context, absolute_deadline)?;
        Ok(owner)
    }
}
