//! Foreground repetition of one owned, cancellable argv Index child.

use super::{
    COMMAND_TIMEOUT, Captured, ChildFailure, ChildOutput, DaemonResponse, MAX_CAPTURE_BYTES,
    Parsed, PreparationRequest, PublicationRequest, RootCommand, canonical_decimal, daemon_path,
    emit_error, path_option, validate_request_selection, validated_response,
};
use std::ffi::{OsStr, OsString};
use std::future::{Future, poll_fn};
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::task::Poll;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

const DEFAULT_INTERVAL_MS: u64 = 30_000;
const CLEANUP_ALLOWANCE: Duration = Duration::from_secs(2);

pub(super) struct Request {
    settings: Vec<OsString>,
    daemon: Option<PathBuf>,
    interval: Duration,
}

pub(super) fn parse(arguments: &[OsString]) -> Result<Parsed, &'static str> {
    let mut settings = Vec::new();
    let mut daemon = None;
    let mut data_root_seen = false;
    let mut config_seen = false;
    let mut interval_ms = None;
    let mut publication = PublicationRequest::default();
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        match argument.to_str() {
            Some("--daemon") => {
                if daemon.is_some() {
                    return Err("USAGE_ERROR");
                }
                daemon = Some(PathBuf::from(path_option(arguments, index)?));
            }
            Some("--data-root" | "--config-file") => {
                let seen = if argument == OsStr::new("--data-root") {
                    &mut data_root_seen
                } else {
                    &mut config_seen
                };
                if *seen {
                    return Err("USAGE_ERROR");
                }
                let value = path_option(arguments, index)?;
                *seen = true;
                settings.push(argument.clone());
                settings.push(value.clone());
            }
            Some("--interval-ms") => {
                if interval_ms.is_some() {
                    return Err("USAGE_ERROR");
                }
                interval_ms = Some(
                    arguments
                        .get(index + 1)
                        .and_then(|value| value.to_str())
                        .and_then(|value| canonical_decimal(value, false))
                        .ok_or("USAGE_ERROR")?,
                );
            }
            _ if publication.take_option(
                RootCommand::Index,
                arguments,
                index,
                &mut settings,
            )? => {}
            _ => return Err("USAGE_ERROR"),
        }
        index += 2;
    }
    validate_request_selection(
        RootCommand::Index,
        false,
        false,
        &PreparationRequest::default(),
        &publication,
    )?;
    let interval = Duration::from_millis(interval_ms.unwrap_or(DEFAULT_INTERVAL_MS));
    Instant::now().checked_add(interval).ok_or("USAGE_ERROR")?;
    Ok(Parsed::Watch(Request {
        settings,
        daemon,
        interval,
    }))
}

async fn register_signal(
    mut signal: Pin<&mut impl Future<Output = io::Result<()>>>,
) -> Result<(), ChildFailure> {
    let early = poll_fn(|cx| {
        Poll::Ready(match signal.as_mut().poll(cx) {
            Poll::Ready(event) => Some(event),
            Poll::Pending => None,
        })
    })
    .await;
    match early {
        None => Ok(()),
        Some(Ok(())) => Err(ChildFailure::Cancelled),
        Some(Err(_)) => Err(ChildFailure::Signal),
    }
}

fn checkpoint(deadline: Instant) -> Result<(), ChildFailure> {
    if Instant::now() >= deadline {
        Err(ChildFailure::Timeout)
    } else {
        Ok(())
    }
}

async fn capture_stream(mut reader: impl AsyncRead + Unpin) -> Result<Captured, ChildFailure> {
    let mut captured = Captured {
        bytes: Vec::with_capacity(MAX_CAPTURE_BYTES),
        exceeded: false,
    };
    let mut buffer = vec![0_u8; 8192];
    loop {
        let count = reader
            .read(&mut buffer)
            .await
            .map_err(|_| ChildFailure::ResultUnknown)?;
        if count == 0 {
            return Ok(captured);
        }
        let keep = MAX_CAPTURE_BYTES
            .saturating_sub(captured.bytes.len())
            .min(count);
        captured.bytes.extend_from_slice(&buffer[..keep]);
        captured.exceeded |= keep < count;
    }
}

async fn exchange(
    child: &mut Child,
    owner: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    deadline: Instant,
    mut signal: Pin<&mut impl Future<Output = io::Result<()>>>,
) -> Result<ChildOutput, ChildFailure> {
    // Taking stdin out of Child prevents wait() from closing this owner pipe.
    let complete = async {
        let (stdout, stderr, status) =
            tokio::try_join!(capture_stream(stdout), capture_stream(stderr), async {
                child.wait().await.map_err(|_| ChildFailure::Wait)
            },)?;
        if stdout.exceeded || stderr.exceeded {
            return Err(ChildFailure::ResultUnknown);
        }
        Ok(ChildOutput {
            status,
            stdout,
            stderr,
        })
    };
    tokio::pin!(complete);
    let result = tokio::select! {
        result = &mut complete => result,
        event = signal.as_mut() => Err(if event.is_ok() {
            ChildFailure::Cancelled
        } else {
            ChildFailure::Signal
        }),
        () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
            Err(ChildFailure::Timeout)
        }
    };
    // Errors close the lifeline and all async reader futures, without detached tasks.
    drop(owner);
    if result.is_ok() {
        checkpoint(deadline)?;
    }
    result
}

async fn finish_child(child: &mut Child) -> Result<(), ChildFailure> {
    // EOF requests cooperative unwinding before the force-kill fallback.
    if matches!(
        tokio::time::timeout(CLEANUP_ALLOWANCE, child.wait()).await,
        Ok(Ok(_))
    ) {
        return Ok(());
    }
    let _ = child.start_kill();
    match tokio::time::timeout(CLEANUP_ALLOWANCE, child.wait()).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) | Err(_) => Err(ChildFailure::Cleanup),
    }
}

async fn capture_index_iteration(
    request: &Request,
    deadline: Instant,
    mut signal: Pin<&mut impl Future<Output = io::Result<()>>>,
) -> Result<ChildOutput, ChildFailure> {
    checkpoint(deadline)?;
    register_signal(signal.as_mut()).await?;
    let daemon = daemon_path(request.daemon.clone()).map_err(|_| ChildFailure::DaemonPath)?;
    let mut command = Command::new(daemon);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
    command
        .arg("--index")
        .arg("--index-owner-stdio")
        .args(&request.settings)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    checkpoint(deadline)?;
    register_signal(signal.as_mut()).await?;
    checkpoint(deadline)?;
    let mut child = command.spawn().map_err(|_| ChildFailure::Start)?;
    let result = async {
        let owner = child.stdin.take().ok_or(ChildFailure::OutputSetup)?;
        let stdout = child.stdout.take().ok_or(ChildFailure::StdoutUnavailable)?;
        let stderr = child.stderr.take().ok_or(ChildFailure::StderrUnavailable)?;
        checkpoint(deadline)?;
        exchange(&mut child, owner, stdout, stderr, deadline, signal).await
    }
    .await;
    if result.is_err() {
        // Only an observed wait qualifies cleanup; kill_on_drop is an unwind backstop.
        finish_child(&mut child).await?;
    }
    result
}

fn iteration_report(result: &Result<ChildOutput, ChildFailure>) -> (String, bool) {
    let output = match result {
        Ok(output) => output,
        Err(failure) => return (format!("{{\"error\":\"{}\"}}", failure.code()), true),
    };
    match (
        output.status.success(),
        validated_response(output, RootCommand::Index, &PreparationRequest::default()),
    ) {
        (true, Some(DaemonResponse::Indexed(line))) => (line.to_owned(), false),
        (false, Some(DaemonResponse::Error(line))) => (line.to_owned(), true),
        _ => ("{\"error\":\"DAEMON_RESULT_UNKNOWN\"}".to_owned(), true),
    }
}

async fn write_bounded(mut writer: impl AsyncWrite + Unpin, line: &str) -> io::Result<()> {
    tokio::time::timeout(CLEANUP_ALLOWANCE, async {
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "watch output deadline"))?
}

async fn write_report(line: &str, error: bool) -> io::Result<()> {
    if error {
        write_bounded(tokio::io::stderr(), line).await
    } else {
        write_bounded(tokio::io::stdout(), line).await
    }
}

async fn signal_exit(event: io::Result<()>) -> i32 {
    if event.is_ok() {
        130
    } else {
        let _ = write_report("{\"error\":\"SIGNAL_UNAVAILABLE\"}", true).await;
        2
    }
}

async fn run_loop(request: Request) -> i32 {
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    loop {
        // Each operation receives one original budget before path/process/pipe setup.
        let Some(deadline) = Instant::now().checked_add(COMMAND_TIMEOUT) else {
            let _ = write_report("{\"error\":\"DEADLINE_UNAVAILABLE\"}", true).await;
            return 2;
        };
        let result = capture_index_iteration(&request, deadline, signal.as_mut()).await;
        if matches!(result.as_ref(), Err(ChildFailure::Cancelled)) {
            return 130;
        }
        let fatal = matches!(
            result.as_ref(),
            Err(ChildFailure::Signal | ChildFailure::Cleanup)
        );
        let (line, error) = iteration_report(&result);
        if fatal {
            let _ = write_report(&line, true).await;
            return 2;
        }
        tokio::select! {
            written = write_report(&line, error) => {
                if written.is_err() {
                    let _ = write_report("{\"error\":\"OUTPUT_WRITE_ERROR\"}", true).await;
                    return 2;
                }
            }
            event = signal.as_mut() => return signal_exit(event).await,
        }
        drop(result);
        let Some(next) = Instant::now().checked_add(request.interval) else {
            let _ = write_report("{\"error\":\"INTERVAL_UNAVAILABLE\"}", true).await;
            return 2;
        };
        // There is no live child or reader future when this after-completion sleep starts.
        tokio::select! {
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(next)) => {}
            event = signal.as_mut() => return signal_exit(event).await,
        }
    }
}

pub(super) fn run(request: Request) -> i32 {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return emit_error("ASYNC_RUNTIME_UNAVAILABLE");
    };
    let result = runtime.block_on(run_loop(request));
    // A cancelled blocked stdio write cannot hold watch shutdown. entry.rs exits
    // the process immediately after all child custody cleanup above has finished.
    runtime.shutdown_background();
    result
}
