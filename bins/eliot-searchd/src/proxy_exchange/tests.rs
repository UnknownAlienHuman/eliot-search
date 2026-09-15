use std::io;

use super::fence::ExchangeFence;
use super::forward::forward_reply;
use super::parser::FATAL_CHILD_FRAMES;
use super::reply::Reply;

struct Disconnected;

impl io::Write for Disconnected {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "injected disconnect",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn disconnect_blocks_next_request_without_consuming_old_reply() {
    let mut fence = ExchangeFence::default();
    let mut old_reply = ["match", "complete"].into_iter();
    let mut reads = 0;
    let result = fence.run(|| {
        forward_reply(
            || {
                reads += 1;
                Ok(old_reply.next().map(str::to_owned))
            },
            &mut Disconnected,
            |line| line == "complete",
            false,
            10,
            1024,
        )
    });
    assert!(result.is_err());
    assert!(fence.blocked());
    assert_eq!(reads, 1);
    let mut new_command_written = false;
    assert!(
        fence
            .run(|| {
                new_command_written = true;
                Ok(Reply::Complete)
            })
            .is_err()
    );
    assert!(!new_command_written);
    assert_eq!(old_reply.next(), Some("complete"));
}

#[test]
fn incomplete_command_write_cannot_be_replayed_automatically() {
    let mut fence = ExchangeFence::default();
    assert!(
        fence
            .run(|| Err("write failed after prefix".to_owned()))
            .is_err()
    );
    assert!(fence.blocked());
    assert!(
        fence
            .run(|| panic!("must not retry uncertain command"))
            .is_err()
    );
}

#[test]
fn complete_response_releases_fence_and_stops_at_its_terminal() {
    let mut lines = ["match", "complete", "next-response"].into_iter();
    let mut output = Vec::new();
    let mut fence = ExchangeFence::default();
    assert_eq!(
        fence
            .run(|| {
                forward_reply(
                    || Ok(lines.next().map(str::to_owned)),
                    &mut output,
                    |line| line == "complete",
                    false,
                    10,
                    1024,
                )
            })
            .unwrap(),
        Reply::Complete
    );
    assert!(!fence.blocked());
    assert_eq!(output, b"match\ncomplete\n");
    assert_eq!(lines.next(), Some("next-response"));
}

#[test]
fn fully_forwarded_command_rejection_does_not_poison_stream() {
    let mut fence = ExchangeFence::default();
    let mut output = Vec::new();
    assert_eq!(
        fence
            .run(|| {
                forward_reply(
                    || Ok(Some("{\"event\":\"error\"}".to_owned())),
                    &mut output,
                    |_| false,
                    false,
                    1,
                    1024,
                )
            })
            .unwrap(),
        Reply::Rejected
    );
    assert!(!fence.blocked());
}

#[test]
fn eof_and_exhausted_line_budget_leave_stream_blocked() {
    for eof in [false, true] {
        let mut fence = ExchangeFence::default();
        assert!(
            fence
                .run(|| {
                    forward_reply(
                        || {
                            Ok(if eof {
                                None
                            } else {
                                Some("not-terminal".to_owned())
                            })
                        },
                        &mut Vec::new(),
                        |_| false,
                        false,
                        2,
                        1024,
                    )
                })
                .is_err()
        );
        assert!(fence.blocked());
    }
}

#[test]
fn shutdown_never_reopens_the_exchange_fence() {
    let mut fence = ExchangeFence::default();
    assert_eq!(
        fence.run(|| Ok(Reply::Shutdown)).unwrap(),
        Reply::Shutdown
    );
    assert!(fence.blocked());
}

#[test]
fn child_fatal_frames_are_forwarded_once_but_never_release_the_exchange() {
    for line in FATAL_CHILD_FRAMES {
        for shutdown in [false, true] {
            let mut fence = ExchangeFence::default();
            let mut output = Vec::new();
            let mut reads = 0;
            assert_eq!(
                fence
                    .run(|| {
                        forward_reply(
                            || {
                                reads += 1;
                                Ok(Some((*line).to_owned()))
                            },
                            &mut output,
                            |_| true,
                            shutdown,
                            3,
                            1024,
                        )
                    })
                    .unwrap(),
                Reply::Fatal
            );
            assert_eq!(reads, 1);
            assert_eq!(output, format!("{line}\n").as_bytes());
            assert!(fence.blocked());
            assert!(
                fence
                    .run(|| panic!("a fatal child must never receive another command"))
                    .is_err()
            );
        }
    }
}

#[test]
fn ordinary_child_validation_error_remains_reusable() {
    let line = r#"{"event":"error","error":"SERVICE_HEX_INVALID"}"#;
    let mut fence = ExchangeFence::default();
    assert_eq!(
        fence
            .run(|| {
                forward_reply(
                    || Ok(Some(line.to_owned())),
                    &mut Vec::new(),
                    |_| true,
                    false,
                    1,
                    1024,
                )
            })
            .unwrap(),
        Reply::Rejected
    );
    assert_eq!(
        fence.run(|| Ok(Reply::Complete)).unwrap(),
        Reply::Complete
    );
    assert!(!fence.blocked());
}

#[test]
fn real_socket_disconnect_mid_large_response_blocks_fence_without_contamination() {
    use std::io::{BufRead, BufReader};
    use std::net::{Ipv4Addr, TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let timeout = Duration::from_secs(5);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let (done, finished) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut stream = stream;
        let mut fence = ExchangeFence::default();
        let result = fence.run(|| {
            let mut step: usize = 0;
            forward_reply(
                || {
                    step += 1;
                    if step <= 2000 {
                        Ok(Some(format!(
                            "match-{step:04}-{}",
                            "x".repeat(1012)
                        )))
                    } else {
                        Ok(Some("END".to_owned()))
                    }
                },
                &mut stream,
                |line| line == "END",
                false,
                3000,
                8 * 1024 * 1024,
            )
        });
        let blocked = fence.blocked();
        let _ = done.send(());
        (result, blocked)
    });
    let client = TcpStream::connect_timeout(&address, timeout).unwrap();
    client.set_read_timeout(Some(timeout)).unwrap();
    let mut reader = BufReader::new(client.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    assert!(first.starts_with("match-0001-"));
    drop(reader);
    drop(client);
    finished
        .recv_timeout(timeout)
        .expect("bounded exchange exit");
    let (result, blocked) = server.join().unwrap();
    assert!(
        result.is_err(),
        "disconnect mid-response must fail, got {result:?}"
    );
    assert!(blocked);

    let mut fence = ExchangeFence::default();
    let mut output = Vec::new();
    let mut fresh = ["{\"event\":\"health\"}", "health-complete"].into_iter();
    assert_eq!(
        fence
            .run(|| {
                forward_reply(
                    || Ok(fresh.next().map(str::to_owned)),
                    &mut output,
                    |line| line == "health-complete",
                    false,
                    10,
                    1024,
                )
            })
            .unwrap(),
        Reply::Complete
    );
    assert!(!fence.blocked());
    assert_eq!(output, b"{\"event\":\"health\"}\nhealth-complete\n");
    assert!(!output.windows(6).any(|window| window == b"match-"));
}

#[test]
fn slow_reader_failure_leaves_fence_blocked_without_replay() {
    struct SlowReader;

    impl io::Write for SlowReader {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::TimedOut))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut fence = ExchangeFence::default();
    let mut lines = ["match-0000", "match-0001", "complete"].into_iter();
    let result = fence.run(|| {
        forward_reply(
            || Ok(lines.next().map(str::to_owned)),
            &mut SlowReader,
            |line| line == "complete",
            false,
            10,
            1024,
        )
    });
    assert!(result.is_err());
    assert!(fence.blocked());
    assert_eq!(lines.next(), Some("match-0001"));
}
