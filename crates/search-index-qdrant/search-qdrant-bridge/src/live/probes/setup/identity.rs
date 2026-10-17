//! Exact live-server version/build probe.

use crate::qualified::{QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION};

use super::super::super::LiveError;
use super::super::super::suite::Suite;

pub(in crate::live) async fn probe_server_identity(suite: &mut Suite<'_>) -> Result<(), LiveError> {
    let reply = suite
        .client
        .health_check()
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let version_ok = reply.version == QUALIFIED_SERVER_VERSION;
    if let Some(build) = reply.commit.as_ref() {
        suite.observe_server_identity(reply.version.clone(), build.clone());
    }
    if let Some(build) = reply.commit.as_ref() {
        suite.observe_server_identity(reply.version.clone(), build.clone());
    }
    let commit_ok = reply
        .commit
        .as_deref()
        .is_some_and(|commit| commit.starts_with(QUALIFIED_SERVER_BUILD));
    suite.record(
        "live_server_identity",
        version_ok && commit_ok,
        format!(
            "health version={} commit={:?} want={QUALIFIED_SERVER_VERSION}/{QUALIFIED_SERVER_BUILD}",
            reply.version, reply.commit,
        ),
    );
    if !version_ok {
        return Err(LiveError::ServerVersionUnexpected);
    }
    if !commit_ok {
        return Err(LiveError::ServerBuildUnexpected);
    }
    Ok(())
}
