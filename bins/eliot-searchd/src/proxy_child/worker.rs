use std::io::{BufReader, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use super::super::exchange::event_name;
use super::model::{Exchange, ExchangeOutput};
use super::pipe::{DeadlineWriter, RequestWriter, read_child_line};
use super::spec::{MAX_LINE_BYTES, MAX_RESPONSE_BYTES, MAX_RESPONSE_LINES};
use super::time::check_request;
use super::{Reply, Terminal, forward_reply};

pub(super) fn spawn_worker(
    mut input: ChildStdin,
    output: ChildStdout,
    ready_tx: SyncSender<Result<(), String>>,
    requests: Receiver<Exchange>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("eliot-direct-pipes".to_owned())
        .spawn(move || {
            let mut output = BufReader::new(output);
            let ready = read_child_line(&mut output).and_then(|line| match line {
                Some(line) if line.starts_with("{\"event\":\"data_root_ready\",") => Ok(()),
                _ => Err("LOOPBACK_DIRECT_CHILD_NOT_READY".to_owned()),
            });
            let valid = ready.is_ok();
            if ready_tx.send(ready).is_err() || !valid {
                return;
            }
            while let Ok(request) = requests.recv() {
                let drain_cancellation = request.drains_cancellation();
                let Exchange {
                    command,
                    socket,
                    terminal,
                    deadline,
                    cancellation,
                    reply,
                } = request;
                // Preserve the pipe boundary for diagnostic reads. Cancellation
                // suppresses their response, not the bounded drain itself.
                let abort_cancellation = if drain_cancellation {
                    None
                } else {
                    cancellation.as_ref()
                };
                let result = (|| {
                    check_request(deadline, abort_cancellation)?;
                    let diagnostic_event = terminal.diagnostic_event(&command)?;
                    // There is only one outstanding child command. Bytes left
                    // after READY or the previous reply are unsolicited, not
                    // the reply to a command that has not been sent yet.
                    if !output.buffer().is_empty() {
                        return Err("LOOPBACK_DIRECT_CHILD_UNSOLICITED_OUTPUT".to_owned());
                    }
                    let mut command_output = RequestWriter {
                        inner: &mut input,
                        deadline,
                        cancellation: abort_cancellation,
                    };
                    command_output
                        .write_all(command.as_bytes())
                        .and_then(|()| command_output.write_all(b"\n"))
                        .and_then(|()| command_output.flush())
                        .map_err(|_| "LOOPBACK_DIRECT_CHILD_WRITE_ERROR".to_owned())?;
                    let mut writer = RequestWriter {
                        inner: DeadlineWriter { socket, deadline },
                        deadline,
                        cancellation: abort_cancellation,
                    };
                    let mut deferred = Vec::new();
                    let shutdown = terminal == Terminal::Shutdown;
                    let mut read = || {
                        check_request(deadline, abort_cancellation)?;
                        let line = read_child_line(&mut output)?;
                        check_request(deadline, abort_cancellation)?;
                        if let (Some(expected), Some(line)) = (diagnostic_event, line.as_deref()) {
                            // Check before forward_reply can write or classify
                            // this line. Real error frames keep their ordinary
                            // rejection/fatal classification in the existing owner.
                            if !matches!(
                                event_name(line),
                                Some(event) if event == expected || event == "error"
                            ) {
                                return Err("LOOPBACK_DIRECT_CHILD_RESPONSE_MISMATCH".to_owned());
                            }
                        }
                        Ok(line)
                    };
                    // Every diagnostic, including bare status, is one bounded
                    // frame. Verify it before any client output, independently
                    // of whether the request supports cancellation. Shutdown
                    // still defers output until actual process exit.
                    let reply = if shutdown || diagnostic_event.is_some() {
                        forward_reply(
                            &mut read,
                            &mut deferred,
                            |line| terminal.reached(line),
                            shutdown,
                            if shutdown { MAX_RESPONSE_LINES } else { 1 },
                            if shutdown {
                                2 * MAX_LINE_BYTES
                            } else {
                                MAX_LINE_BYTES
                            },
                        )?
                    } else {
                        forward_reply(
                            &mut read,
                            &mut writer,
                            |line| terminal.reached(line),
                            false,
                            MAX_RESPONSE_LINES,
                            MAX_RESPONSE_BYTES,
                        )?
                    };
                    check_request(deadline, abort_cancellation)?;
                    // The child cannot legitimately answer another request yet.
                    // Reject even a partial prefetched suffix; never clear/drain
                    // it and then reuse a stream with an uncertain boundary.
                    if !output.buffer().is_empty() {
                        return Err("LOOPBACK_DIRECT_CHILD_UNSOLICITED_OUTPUT".to_owned());
                    }
                    Ok(ExchangeOutput { reply, deferred })
                })();
                let reusable = result.as_ref().is_ok_and(|output| {
                    matches!(output.reply, Reply::Complete | Reply::Rejected)
                });
                if reply.send(result).is_err() || !reusable {
                    break;
                }
            }
            // Dropping input closes stdin. No shutdown is written after
            // uncertainty.
        })
        .map_err(|_| "LOOPBACK_DIRECT_PIPE_WORKER_START_ERROR".to_owned())
}
