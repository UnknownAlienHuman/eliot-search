//! MCP stdio entrypoint over the existing owned-child search SDK.
#![forbid(unsafe_code)]

mod server;
mod window;
mod wire;

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::ExitCode;

use rmcp::ServiceExt;

#[derive(Clone)]
pub(crate) struct Config {
    daemon: PathBuf,
    root: PathBuf,
    qdrant: Option<String>,
}

impl Config {
    fn arguments(&self, session: bool, ranked: bool) -> Result<Vec<OsString>, &'static str> {
        let mut args = vec![
            OsString::from(if session {
                "--session-stdio"
            } else {
                "--search-stdio"
            }),
            OsString::from("--data-root"),
            self.root.as_os_str().to_owned(),
        ];
        if ranked {
            let endpoint = self.qdrant.as_ref().ok_or("MCP_QDRANT_URL_REQUIRED")?;
            args.extend([OsString::from("--qdrant-url"), OsString::from(endpoint)]);
        }
        Ok(args)
    }
}

fn arguments() -> Result<Option<Config>, &'static str> {
    let values: Vec<_> = std::env::args_os().skip(1).collect();
    if values.len() == 1 && values[0] == OsStr::new("--help") {
        eprintln!("eliot-search-mcp --data-root ROOT [--daemon PATH] [--qdrant-url URL]");
        return Ok(None);
    }
    if values.len() == 1 && values[0] == OsStr::new("--version") {
        eprintln!("eliot-search-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    let (mut root, mut daemon, mut qdrant) = (None, None, None);
    let mut args = values.into_iter();
    while let Some(option) = args.next() {
        let value = args.next().ok_or("MCP_USAGE_ERROR")?;
        if option == OsStr::new("--data-root") && root.is_none() {
            root = Some(PathBuf::from(value));
        } else if option == OsStr::new("--daemon") && daemon.is_none() {
            daemon = Some(PathBuf::from(value));
        } else if option == OsStr::new("--qdrant-url") && qdrant.is_none() {
            qdrant = Some(value.into_string().map_err(|_| "MCP_ENDPOINT_INVALID")?);
        } else {
            return Err("MCP_USAGE_ERROR");
        }
    }
    let root = root.ok_or("MCP_DATA_ROOT_REQUIRED")?;
    if !root.is_absolute() {
        return Err("MCP_DATA_ROOT_MUST_BE_ABSOLUTE");
    }
    let daemon = if let Some(path) = daemon {
        path
    } else {
        let executable = std::env::current_exe().map_err(|_| "MCP_EXECUTABLE_UNAVAILABLE")?;
        executable
            .parent()
            .ok_or("MCP_EXECUTABLE_UNAVAILABLE")?
            .join(if cfg!(windows) {
                "eliot-searchd.exe"
            } else {
                "eliot-searchd"
            })
    };
    Ok(Some(Config {
        daemon,
        root,
        qdrant,
    }))
}

struct ExitState {
    result: Result<(), &'static str>,
    children_reaped: bool,
}

async fn run(config: Config) -> ExitState {
    let wire = std::sync::Arc::new(wire::Wire::new());
    let (server, owners) = match server::McpServer::start(config, wire.clone()) {
        Ok(started) => started,
        Err(code) => {
            return ExitState {
                result: Err(code),
                children_reaped: true,
            };
        }
    };
    let (input, input_task) = wire::input(wire.clone());
    let service_wire = wire.clone();
    let mut service_task = tokio::spawn(async move {
        let transport = wire::RpcTransport::new(input, service_wire.clone());
        match server
            .serve_with_ct(transport, service_wire.shutdown.clone())
            .await
        {
            Ok(running) => running
                .waiting()
                .await
                .map(|_| ())
                .map_err(|_| "MCP_SERVICE_FAILED"),
            Err(_) => Err("MCP_INITIALIZE_FAILED"),
        }
    });
    let mut service_result = None;
    tokio::select! {
        biased;
        () = wire.shutdown.cancelled() => {}
        completed = &mut service_task => {
            service_result = Some(completed.map_err(|_| "MCP_SERVICE_PANICKED").and_then(|result|result));
            wire.shutdown.cancel();
        }
    }

    // This join/finish does not depend on a writable MCP stdout or rmcp close.
    // No transport timeout is a substitute for this actual product cleanup.
    let cleaned = owners.finish().await;
    wire.cleanup_complete(cleaned);
    let children_reaped = cleaned.is_ok();
    if !children_reaped {
        eprintln!("MCP_OWNER_CLEANUP_FAILED");
        // Without positive SDK cleanup evidence do not claim bounded teardown.
        if service_result.is_none() {
            let _ = service_task.await;
        }
        let _ = input_task.await;
        return ExitState {
            result: cleaned,
            children_reaped: false,
        };
    }

    let service_result = if let Some(result) = service_result {
        result
    } else if let Ok(completed) =
        tokio::time::timeout(wire::STDIO_TEARDOWN, &mut service_task).await
    {
        completed
            .map_err(|_| "MCP_SERVICE_PANICKED")
            .and_then(|result| result)
    } else {
        wire.fail("MCP_TRANSPORT_SHUTDOWN_BLOCKED");
        service_task.abort();
        let _ = service_task.await;
        Err("MCP_TRANSPORT_SHUTDOWN_BLOCKED")
    };
    // Only standard-stream work remains. Its blocking Tokio jobs may require
    // bounded runtime teardown after every SDK child has already been reaped.
    input_task.abort();
    let _ = input_task.await;
    let failure = wire
        .failure()
        .and_then(|failure| failure.map_or(Ok(()), Err));
    drop(wire);
    ExitState {
        result: cleaned.and(service_result).and(failure),
        children_reaped,
    }
}

fn main() -> ExitCode {
    let config = match arguments() {
        Ok(Some(config)) => config,
        Ok(None) => return ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}");
            return ExitCode::from(2);
        }
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        eprintln!("MCP_RUNTIME_UNAVAILABLE");
        return ExitCode::from(2);
    };
    let state = runtime.block_on(run(config));
    let status = match state.result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}");
            ExitCode::from(2)
        }
    };
    if state.children_reaped {
        runtime.shutdown_timeout(wire::STDIO_TEARDOWN);
    } else {
        drop(runtime);
    }
    status
}
