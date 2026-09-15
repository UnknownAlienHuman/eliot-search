use std::io::{BufReader, Write};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use super::model::{Exchange, ExchangeOutput};
use super::pipe::{DeadlineWriter, read_child_line};
use super::spec::{MAX_LINE_BYTES, MAX_RESPONSE_BYTES, MAX_RESPONSE_LINES};
use super::time::remaining;
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
                let Exchange {
                    command,
                    socket,
                    terminal,
                    deadline,
                    reply,
                } = request;
                let result = (|| {
                    remaining(deadline)?;
                    input
                        .write_all(command.as_bytes())
                        .and_then(|()| input.write_all(b"\n"))
                        .and_then(|()| input.flush())
                        .map_err(|_| "LOOPBACK_DIRECT_CHILD_WRITE_ERROR".to_owned())?;
                    let mut writer = DeadlineWriter { socket, deadline };
                    let mut deferred = Vec::new();
                    let shutdown = terminal == Terminal::Shutdown;
                    let mut read = || {
                        remaining(deadline)?;
                        read_child_line(&mut output)
                    };
                    // Do not send a clean-stop frame before observing actual
                    // process exit. Retain the child's exact bytes, not a
                    // synthetic receipt.
                    let reply = if shutdown {
                        forward_reply(
                            &mut read,
                            &mut deferred,
                            |line| terminal.reached(line),
                            true,
                            MAX_RESPONSE_LINES,
                            2 * MAX_LINE_BYTES,
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
                    remaining(deadline)?;
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
