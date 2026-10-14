//! Owning service context/terminal cases, AUTHORED_NOT_COMPILED/NOT_RUN for Q215.

use super::*;
use search_runtime_owner::DataRootOpenMode;
use std::ffi::OsString;
use std::io::Write;
use std::time::Duration;

fn request(io: usize, calls: usize, flag: Arc<AtomicBool>) -> NativeRootRequest {
    NativeRootRequest::new(
        DataRootOpenMode::InspectExistingReadOnly,
        &[
            OsString::from("--serve-data-root"),
            OsString::from("private-root"),
        ],
        Instant::now() + Duration::from_secs(120),
        flag,
        super::super::super::native_request::NativeRootLimits::new([
            256,
            256 * 1024,
            512 * 1024,
            io,
            calls,
            256,
        ])
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn service_request_preserves_full_argv_and_independent_command_context() {
    let startup_flag = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(120);
    let args = [
        OsString::from("--config-file"),
        OsString::from("private-config"),
        OsString::from("--serve-data-root"),
        OsString::from("private-root"),
    ];
    let startup = NativeRootRequest::for_service(
        &args,
        deadline,
        Arc::clone(&startup_flag),
        request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits,
    )
    .unwrap();
    assert_eq!(startup.root(), std::path::Path::new("private-root"));
    let original = startup.arguments().to_vec();
    assert_eq!(original.len(), args.len());
    let command_flag = Arc::new(AtomicBool::new(false));
    let command_deadline = deadline + Duration::from_secs(1);
    startup_flag.store(true, Ordering::Release);
    let command = startup
        .service_command("health", command_deadline, Arc::clone(&command_flag))
        .unwrap();
    assert_eq!(&command.arguments()[..original.len()], original.as_slice());
    assert_eq!(command.arguments().len(), original.len() + 1);
    assert_eq!(command.root(), startup.root());
    assert_eq!(command.credential_context().0, command_deadline);
    assert!(std::ptr::eq(command.credential_context().1, &*command_flag));
    assert_ne!(command.binding().0, startup.binding().0);
    assert_eq!(command.check(), Ok(()));
    command_flag.store(true, Ordering::Release);
    assert_eq!(
        command.check(),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
}

#[cfg(windows)]
#[test]
fn native_service_keeps_read_custody_and_original_shutdown_through_release() {
    use super::super::super::{NativeRootAdmission, admit_existing, initialize_new};
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "search-read-service-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let limits = request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits;
    let init = NativeRootRequest::new(
        DataRootOpenMode::InitializeNew,
        &[
            OsString::from("--initialize-data-root"),
            root.as_os_str().to_owned(),
            OsString::from("02020202020202020202020202020202"),
        ],
        Instant::now() + Duration::from_secs(120),
        Arc::new(AtomicBool::new(false)),
        limits,
    )
    .unwrap();
    initialize_new(init).unwrap().emit(&mut Vec::new()).unwrap();
    let startup_flag = Arc::new(AtomicBool::new(false));
    let req = NativeRootRequest::for_service(
        &[
            OsString::from("--serve-data-root"),
            root.as_os_str().to_owned(),
        ],
        Instant::now() + Duration::from_secs(120),
        Arc::clone(&startup_flag),
        limits,
    )
    .unwrap();
    let long_refusal = format!("index-file\t{}", "a".repeat(131072));
    assert!(long_refusal.len() < 256 * 1024);
    assert!(
        req.service_command(
            &long_refusal,
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
        )
        .is_err()
    );
    let NativeRootAdmission::Inspection(owner) = admit_existing(req).unwrap() else {
        panic!("inspection")
    };
    let before: Vec<_> = [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ]
    .map(|name| std::fs::read(root.join(name)).unwrap())
    .into();
    let startup = owner.read_startup().unwrap();
    let mut service = startup.finish().unwrap();
    startup_flag.store(true, Ordering::Release);
    for (command, code) in [
        ("gc\tbad\tbad", "SERVICE_MUTATION_UNAVAILABLE"),
        ("", "SERVICE_COMMAND_INVALID"),
        (long_refusal.as_str(), "SERVICE_MUTATION_UNAVAILABLE"),
    ] {
        let mut output = Vec::new();
        service
            .refuse_command(
                command,
                Instant::now() + Duration::from_secs(120),
                Arc::new(AtomicBool::new(false)),
                &mut output,
                code,
            )
            .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!("{{\"event\":\"error\",\"error\":\"{code}\"}}\n")
        );
    }
    let health = service
        .read_command(
            "health",
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    health.native_checkpoint().unwrap();
    drop(health);
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(120);
    let child = service
        .read_command("shutdown", deadline, Arc::clone(&shutdown_flag))
        .unwrap();
    child.write_output(&mut Vec::new(), b"draining\n").unwrap();
    let retained = child.retain_shutdown().unwrap();
    let binding = retained.request.binding();
    assert_eq!(retained.request.credential_context().0, deadline);
    assert!(std::ptr::eq(
        retained.request.credential_context().1,
        &*shutdown_flag
    ));
    assert!(retained.output_attempted);
    let closed = service.close();
    assert!(closed.owner.bound_root.metadata().is_ok());
    assert_eq!(closed.owner.exclusions.len(), 2);
    let terminal = closed.finish(retained).unwrap();
    assert_eq!(terminal.request.binding(), binding);
    assert_eq!(terminal.request.credential_context().0, deadline);
    assert!(std::ptr::eq(
        terminal.request.credential_context().1,
        &*shutdown_flag
    ));
    assert!(terminal.output_attempted);
    shutdown_flag.store(true, Ordering::Release);
    let mut output = Vec::new();
    assert_eq!(
        terminal.emit(&mut output),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert!(output.is_empty());
    let after: Vec<_> = [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ]
    .map(|name| std::fs::read(root.join(name)).unwrap())
    .into();
    assert_eq!(before, after);
    // Qualification retains this root and its credential; no blind deletion.
}

#[derive(Default)]
struct ShortOutput {
    requests: Vec<usize>,
    flushes: usize,
    cancel_on_flush: Option<Arc<AtomicBool>>,
}

impl Write for ShortOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.requests.push(bytes.len());
        Ok(bytes.len().min(3))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.flushes += 1;
        if let Some(flag) = &self.cancel_on_flush {
            flag.store(true, Ordering::Release);
        }
        Ok(())
    }
}

#[test]
fn terminal_short_writes_and_flush_use_the_retained_cumulative_meter() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let req = request(1024 * 1024, 16384, Arc::new(AtomicBool::new(false)));
    let mut work = NativeWork::new();
    work.write(&req, 17).unwrap();
    let trace = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&trace);
    work.usage_hook = Some(Box::new(move |phase, used, bytes| {
        captured.borrow_mut().push((phase, used, bytes));
    }));
    let terminal = NativeReadTerminal {
        request: req,
        work,
        output_attempted: false,
    };
    let mut output = ShortOutput::default();
    assert_eq!(terminal.emit(&mut output), Ok(()));
    let trace = trace.borrow();
    let reservations: Vec<_> = trace
        .iter()
        .filter(|row| row.0 == "write-reservation")
        .map(|row| row.2)
        .collect();
    assert_eq!(reservations, output.requests);
    assert!(output.requests.windows(2).any(|pair| pair[0] > pair[1]));
    assert_eq!(output.flushes, 1);
    let completed = trace
        .iter()
        .find(|row| row.0 == "read-terminal-complete")
        .unwrap();
    assert_eq!(completed.1[0], 17 + output.requests.iter().sum::<usize>());
    assert_eq!(completed.1[1], 1 + output.requests.len() + output.flushes);
}

#[test]
fn exhausted_terminal_allowance_never_dispatches_output() {
    for (bytes, calls) in [(1, 16384), (1024 * 1024, 1)] {
        let terminal = NativeReadTerminal {
            request: request(bytes, calls, Arc::new(AtomicBool::new(false))),
            work: NativeWork::new(),
            output_attempted: false,
        };
        let mut output = ShortOutput::default();
        assert_eq!(
            terminal.emit(&mut output),
            Err(OwnerError::ContractExhausted)
        );
        assert!(output.requests.is_empty());
        assert_eq!(output.flushes, 0);
    }
}

#[test]
fn terminal_flush_cancellation_and_prior_output_keep_unknown_outcome() {
    let flag = Arc::new(AtomicBool::new(false));
    let terminal = NativeReadTerminal {
        request: request(1024 * 1024, 16384, Arc::clone(&flag)),
        work: NativeWork::new(),
        output_attempted: false,
    };
    let mut output = ShortOutput {
        cancel_on_flush: Some(flag),
        ..ShortOutput::default()
    };
    assert_eq!(
        terminal.emit(&mut output),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert_eq!(output.flushes, 1);
    let flag = Arc::new(AtomicBool::new(false));
    let terminal = NativeReadTerminal {
        request: request(1024 * 1024, 16384, Arc::clone(&flag)),
        work: NativeWork::new(),
        output_attempted: true,
    };
    flag.store(true, Ordering::Release);
    let mut output = ShortOutput::default();
    assert_eq!(
        terminal.emit(&mut output),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert!(output.requests.is_empty());
}

#[test]
fn one_shot_request_preserves_native_config_argv_and_original_context() {
    let flag = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(120);
    let args = [
        OsString::from("--config-file"),
        OsString::from("private-config"),
        OsString::from("--read-revision"),
        OsString::from("private-root"),
        OsString::from("private-revision"),
        OsString::from("0"),
        OsString::from("8"),
    ];
    let limits = request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits;
    let (invocation, selected) =
        NativeRootRequest::for_read_one_shot(&args, deadline, Arc::clone(&flag), limits).unwrap();
    assert_eq!(invocation.root(), std::path::Path::new("private-root"));
    assert_eq!(invocation.arguments().len(), 7);
    assert_eq!(selected, args[2..]);
    assert_eq!(invocation.credential_context().0, deadline);
    assert!(std::ptr::eq(invocation.credential_context().1, &*flag));
    flag.store(true, Ordering::Release);
    assert_eq!(
        invocation.check(),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
    let bad = [
        OsString::from("--index-file"),
        OsString::from("not-created"),
    ];
    assert!(
        NativeRootRequest::for_read_one_shot(
            &bad,
            deadline,
            Arc::new(AtomicBool::new(false)),
            limits,
        )
        .is_err()
    );
}

#[cfg(windows)]
#[test]
fn one_shot_output_keeps_original_context_closes_children_and_preserves_owner_slots() {
    use super::super::super::{NativeRootAdmission, admit_existing, initialize_new};
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "search-one-shot-read-{}-{stamp}",
        std::process::id(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let limits = request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits;
    let init = NativeRootRequest::new(
        DataRootOpenMode::InitializeNew,
        &[
            OsString::from("--initialize-data-root"),
            root.as_os_str().to_owned(),
            OsString::from("03030303030303030303030303030303"),
        ],
        Instant::now() + Duration::from_secs(120),
        Arc::new(AtomicBool::new(false)),
        limits,
    )
    .unwrap();
    initialize_new(init).unwrap().emit(&mut Vec::new()).unwrap();
    let slots = || {
        [
            ".eliot-search-owner-state-a.v1",
            ".eliot-search-owner-state-b.v1",
        ]
        .map(|name| std::fs::read(root.join(name)).unwrap())
    };
    let before = slots();
    for cancel_on_flush in [false, true] {
        let flag = Arc::new(AtomicBool::new(false));
        let args = [
            OsString::from("--health-data-root"),
            root.as_os_str().to_owned(),
        ];
        let (request, _) = NativeRootRequest::for_read_one_shot(
            &args,
            Instant::now() + Duration::from_secs(120),
            Arc::clone(&flag),
            limits,
        )
        .unwrap();
        let NativeRootAdmission::Inspection(owner) = admit_existing(request).unwrap() else {
            panic!("inspection");
        };
        let startup = owner.read_startup().unwrap();
        let mut sink = ShortOutput {
            cancel_on_flush: cancel_on_flush.then(|| Arc::clone(&flag)),
            ..ShortOutput::default()
        };
        let result = startup.run_one_shot(|child| {
            use crate::direct_store::DirectRead;
            child.verify()?;
            let competing = NativeRootRequest::new(
                DataRootOpenMode::InspectExistingReadOnly,
                &args,
                Instant::now() + Duration::from_secs(120),
                Arc::new(AtomicBool::new(false)),
                limits,
            )
            .unwrap();
            assert!(admit_existing(competing).is_err());
            let mut output = crate::direct_store::NativeOutput::new(child, &mut sink);
            output
                .write_all(b"verified\n")
                .map_err(|_| "OUTPUT_FAILED".to_owned())?;
            output.flush().map_err(|_| "OUTPUT_FAILED".to_owned())
        });
        if cancel_on_flush {
            assert_eq!(
                result,
                Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned())
            );
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(sink.requests, vec![9, 6, 3]);
        assert_eq!(sink.flushes, 1);
        assert_eq!(slots(), before);
        let (request, _) = NativeRootRequest::for_read_one_shot(
            &args,
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
            limits,
        )
        .unwrap();
        assert!(matches!(
            admit_existing(request).unwrap(),
            NativeRootAdmission::Inspection(_)
        ));
    }
    // Qualification retains the native root and credential; no blind cleanup.
}

#[test]
fn refusal_output_cannot_reset_consumed_admission_work() {
    let req = request(1024 * 1024, 2, Arc::new(AtomicBool::new(false)));
    let mut work = NativeWork::new();
    work.call(&req, || Ok(())).unwrap();
    let req = req.retain_refusal_work(work);
    let mut output = ShortOutput::default();
    assert_eq!(
        req.emit(&mut output, "{}"),
        Err(OwnerError::ContractExhausted)
    );
    assert!(output.requests.is_empty());
    assert_eq!(output.flushes, 0);
}

#[test]
fn refusal_context_cannot_revive_after_cancellation_is_cleared() {
    let flag = Arc::new(AtomicBool::new(false));
    let req = request(1024 * 1024, 16384, Arc::clone(&flag));
    let mut output = ShortOutput::default();
    flag.store(true, Ordering::Release);
    assert_eq!(
        req.emit(&mut output, "{}"),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
    flag.store(false, Ordering::Release);
    assert_eq!(
        req.emit(&mut output, "{}"),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
    assert!(output.requests.is_empty());
    assert_eq!(output.flushes, 0);
}

#[test]
fn refusal_flush_uncertainty_cannot_be_replayed_after_flag_clear() {
    let flag = Arc::new(AtomicBool::new(false));
    let req = request(1024 * 1024, 16384, Arc::clone(&flag));
    let mut output = ShortOutput {
        cancel_on_flush: Some(Arc::clone(&flag)),
        ..ShortOutput::default()
    };
    assert_eq!(
        req.emit(&mut output, "{}"),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    let calls = output.requests.clone();
    flag.store(false, Ordering::Release);
    assert_eq!(
        req.emit(&mut output, "{}"),
        Err(OwnerError::OwnerAcquireOutcomeUnknown)
    );
    assert_eq!(output.requests, calls);
    assert_eq!(output.flushes, 1);
}

#[test]
fn registration_selector_preserves_native_config_context_and_rejects_arity() {
    let flag = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(120);
    let args = [
        OsString::from("--config-file"),
        OsString::from("private-config"),
        OsString::from("--source-roots"),
        OsString::from("private-root"),
    ];
    let limits = request(1024 * 1024 * 1024, 16384, Arc::clone(&flag)).limits;
    let (request, selected) =
        NativeRootRequest::for_read_one_shot(&args, deadline, Arc::clone(&flag), limits).unwrap();
    assert_eq!(selected, args[2..]);
    assert_eq!(request.arguments().len(), args.len());
    assert_eq!(request.credential_context().0, deadline);
    assert!(std::ptr::eq(request.credential_context().1, &*flag));
    for bad in [
        vec![OsString::from("--source-roots")],
        vec![
            OsString::from("--source-roots"),
            OsString::from("private-root"),
            OsString::from("extra"),
        ],
    ] {
        assert!(
            NativeRootRequest::for_read_one_shot(&bad, deadline, Arc::clone(&flag), limits)
                .is_err()
        );
    }
    flag.store(true, Ordering::Release);
    assert_eq!(
        request.check(),
        Err(OwnerError::OwnerCancelledBeforeMutation)
    );
}

#[cfg(windows)]
#[test]
fn registration_catalog_refuses_corrupt_and_pending_bytes_without_recovery_or_writes() {
    use super::super::super::{NativeRootAdmission, admit_existing, initialize_new};
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "search-registration-read-{}-{stamp}",
        std::process::id()
    ));
    let absent = root.with_extension("absent");
    let limits = request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits;
    let invocation = |root: &std::path::Path, flag: Arc<AtomicBool>| {
        NativeRootRequest::for_read_one_shot(
            &[
                OsString::from("--source-roots"),
                root.as_os_str().to_owned(),
            ],
            Instant::now() + Duration::from_secs(120),
            flag,
            limits,
        )
        .unwrap()
        .0
    };
    assert!(matches!(
        admit_existing(invocation(&absent, Arc::new(AtomicBool::new(false)))).unwrap(),
        NativeRootAdmission::Unavailable(_, _)
    ));
    assert!(!absent.exists());
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    initialize_new(
        NativeRootRequest::new(
            DataRootOpenMode::InitializeNew,
            &[
                OsString::from("--initialize-data-root"),
                root.as_os_str().to_owned(),
                OsString::from("04040404040404040404040404040404"),
            ],
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
            limits,
        )
        .unwrap(),
    )
    .unwrap()
    .emit(&mut Vec::new())
    .unwrap();
    let catalog = root.join("control/source-roots.v1");
    let sources = root.with_extension("sources");
    std::fs::create_dir(&sources).unwrap();
    let sources = std::fs::canonicalize(sources).unwrap();
    let available = sources.join("a-available");
    std::fs::create_dir(&available).unwrap();
    let available = std::fs::canonicalize(available).unwrap();
    let missing = sources.join("b-missing");
    let not_directory = sources.join("c-file");
    std::fs::write(&not_directory, b"retained-source").unwrap();
    let not_directory = std::fs::canonicalize(not_directory).unwrap();
    let nonempty = format!(
        "# ELIOT Search source roots v1\n{}\n{}\n{}\n",
        not_directory.to_str().unwrap(),
        missing.to_str().unwrap(),
        available.to_str().unwrap(),
    );
    let slots = [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ]
    .map(|name| std::fs::read(root.join(name)).unwrap());
    for (bytes, sidecar, expected) in [
        (None, None, None),
        (Some(nonempty.as_bytes()), None, None),
        (
            Some(b"# ELIOT Search source roots v1\n".as_slice()),
            None,
            None,
        ),
        (
            Some(b"corrupt\n".as_slice()),
            None,
            Some("SOURCE_ROOT_CATALOG_CORRUPT"),
        ),
        (
            Some(b"# ELIOT Search source roots v1\n".as_slice()),
            Some("bak"),
            Some("SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN"),
        ),
        (
            Some(b"# ELIOT Search source roots v1\n".as_slice()),
            Some("tmp"),
            Some("SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN"),
        ),
    ] {
        if catalog.exists() {
            std::fs::remove_file(&catalog).unwrap();
        }
        if let Some(bytes) = bytes {
            std::fs::write(&catalog, bytes).unwrap();
        }
        if let Some(extension) = sidecar {
            std::fs::write(catalog.with_extension(extension), b"retained-pending").unwrap();
        }
        let NativeRootAdmission::Inspection(owner) =
            admit_existing(invocation(&root, Arc::new(AtomicBool::new(false)))).unwrap()
        else {
            panic!("inspection");
        };
        let result = owner.run_registration_one_shot(|operation| {
            let result = crate::source_roots::SourceRootInspection::read(operation);
            if let Some(code) = expected {
                assert!(matches!(&result, Err(error) if error == code));
                result.map(|_| ())
            } else {
                let output = result?.output()?;
                let text = String::from_utf8(output.clone()).unwrap();
                if bytes == Some(nonempty.as_bytes()) {
                    let paths = [&available, &missing, &not_directory];
                    let states = ["available", "missing", "not_directory"];
                    let mut expected_rows: Vec<_> = paths.iter().zip(states).enumerate().map(
                        |(position, (path, state))| format!(
                            "{{\"event\":\"source_root\",\"position\":{position},\"path\":{},\"state\":\"{state}\"}}",
                            crate::service_output::json_string(path.to_str().unwrap()),
                        ),
                    ).collect();
                    expected_rows.extend([
                        "{\"event\":\"source_gap\",\"position\":1,\"reason\":\"OBSERVATION_GAP_MISSING\",\"state\":\"missing\"}".to_owned(),
                        "{\"event\":\"source_gap\",\"position\":2,\"reason\":\"OBSERVATION_GAP_NOT_DIRECTORY\",\"state\":\"not_directory\"}".to_owned(),
                        concat!(
                            "{\"event\":\"source_roots_complete\",\"configured\":3,\"available\":1,",
                            "\"unavailable\":2,\"gaps\":2,\"reconciliation_generation\":1,",
                            "\"watcher_pending\":0,\"watcher_overflowed\":false,",
                            "\"proven_reason\":\"ROOTS_GAP_BLOCKS_CURRENTNESS\",\"current_workspace_proven\":false}"
                        ).to_owned(),
                    ]);
                    assert_eq!(text, expected_rows.join("\n") + "\n");
                } else {
                    assert!(text.contains("\"configured\":0"));
                }
                operation.emit_registration(&mut Vec::new(), &output)
            }
        });
        if let Some(code) = expected {
            assert_eq!(result, Err(code.to_owned()));
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(std::fs::read(&catalog).ok().as_deref(), bytes);
        for (name, before) in [
            ".eliot-search-owner-state-a.v1",
            ".eliot-search-owner-state-b.v1",
        ]
        .into_iter()
        .zip(&slots)
        {
            assert_eq!(&std::fs::read(root.join(name)).unwrap(), before);
        }
        if let Some(extension) = sidecar {
            assert_eq!(
                std::fs::read(catalog.with_extension(extension)).unwrap(),
                b"retained-pending"
            );
            std::fs::remove_file(catalog.with_extension(extension)).unwrap();
        }
    }
    // A migration reservation precedes external I/O; its post-call checkpoint
    // must still refuse pending registration or the same original cancellation.
    // Reuse this real initialized root instead of adding another initializer.
    for cancel in [false, true] {
        use search_revision_store::LegacyRevisionObjectWork as Work;
        std::fs::write(&catalog, b"# ELIOT Search source roots v1\n").unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let req = invocation(&root, Arc::clone(&flag));
        let original_deadline = req.credential_context().0;
        let NativeRootAdmission::Inspection(owner) = admit_existing(req).unwrap() else {
            panic!("inspection");
        };
        let mut output = Vec::new();
        let expected = if cancel {
            OwnerError::OwnerCancelledBeforeMutation
        } else {
            OwnerError::OwnerRecoveryQuarantined
        };
        let result = owner.run_registration_one_shot(|operation| {
            operation.registration_bytes()?;
            operation.migration_charge(Work::Read(1)).map_err(|error| error.code().to_owned())?;
            if cancel {
                flag.store(true, Ordering::Release);
            } else {
                std::fs::write(catalog.with_extension("tmp"), b"adversarial-pending").unwrap();
            }
            assert_eq!(operation.migration_checkpoint(), Err(expected));
            // Disappearance of the external cause must not renew this request.
            if cancel {
                flag.store(false, Ordering::Release);
            } else {
                std::fs::remove_file(catalog.with_extension("tmp")).unwrap();
            }
            assert_eq!(operation.migration_charge(Work::Allocate(1)), Err(expected));
            assert_eq!(operation.migration_checkpoint(), Err(expected));
            assert_eq!(operation.migration_deadline(), original_deadline);
            assert_eq!(operation.emit_registration(&mut output, b"must not escape\n"),
                Err(expected.code().to_owned()));
            assert!(!operation.output_attempted());
            // Swallowing the child error cannot make final custody checks succeed.
            Ok(())
        });
        assert_eq!(result, Err(expected.code().to_owned()));
        assert!(output.is_empty());
        assert_eq!(std::fs::read(&catalog).unwrap(), b"# ELIOT Search source roots v1\n");
        for (name, before) in [".eliot-search-owner-state-a.v1", ".eliot-search-owner-state-b.v1"]
            .into_iter().zip(&slots) {
            assert_eq!(&std::fs::read(root.join(name)).unwrap(), before);
        }
    }
    // Retain native root/credential evidence; qualification owns cleanup.
}

#[cfg(windows)]
#[test]
fn registration_output_retains_pin_original_work_and_terminal_cancellation() {
    use super::super::super::{NativeRootAdmission, admit_existing, initialize_new};
    use std::time::{SystemTime, UNIX_EPOCH};
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "search-registration-terminal-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let limits = request(1024 * 1024 * 1024, 16384, Arc::new(AtomicBool::new(false))).limits;
    let args = [
        OsString::from("--source-roots"),
        root.as_os_str().to_owned(),
    ];
    initialize_new(
        NativeRootRequest::new(
            DataRootOpenMode::InitializeNew,
            &[
                OsString::from("--initialize-data-root"),
                root.as_os_str().to_owned(),
                OsString::from("05050505050505050505050505050505"),
            ],
            Instant::now() + Duration::from_secs(120),
            Arc::new(AtomicBool::new(false)),
            limits,
        )
        .unwrap(),
    )
    .unwrap()
    .emit(&mut Vec::new())
    .unwrap();
    let catalog = root.join("control/source-roots.v1");
    std::fs::write(&catalog, b"# ELIOT Search source roots v1\n").unwrap();
    for mode in [0, 1, 2] {
        let flag = Arc::new(AtomicBool::new(false));
        let invocation = || {
            NativeRootRequest::for_read_one_shot(
                &args,
                Instant::now() + Duration::from_secs(120),
                Arc::clone(&flag),
                limits,
            )
            .unwrap()
            .0
        };
        let NativeRootAdmission::Inspection(owner) = admit_existing(invocation()).unwrap() else {
            panic!("inspection");
        };
        let mut sink = ShortOutput {
            cancel_on_flush: (mode == 1).then(|| Arc::clone(&flag)),
            ..ShortOutput::default()
        };
        let result = owner.run_registration_one_shot(|operation| {
            assert!(admit_existing(invocation()).is_err());
            assert_eq!(
                operation.registration_bytes()?,
                Some(b"# ELIOT Search source roots v1\n".to_vec())
            );
            // The retained catalog excludes writes before every output and teardown.
            assert!(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&catalog)
                    .is_err()
            );
            if mode == 2 {
                use std::cell::Cell;
                use std::rc::Rc;
                let used = Rc::new(Cell::new(0));
                let captured = Rc::clone(&used);
                operation
                    .with_binding(|_, request, work, _| {
                        work.usage_hook = Some(Box::new(move |_, usage, _| captured.set(usage[0])));
                        work.arrive(request, "registration-original-meter")?;
                        work.read(request, 1024 * 1024 * 1024 - used.get())?;
                        work.usage_hook = None;
                        Ok(())
                    })
                    .map_err(|error| error.code().to_owned())?;
                assert!(operation.owner.poisoned.is_none());
            }
            operation.emit_registration(&mut sink, b"verified\n")
        });
        match mode {
            0 => {
                assert_eq!(result, Ok(()));
                assert_eq!(sink.requests, vec![9, 6, 3]);
            }
            1 => assert_eq!(
                result,
                Err(OwnerError::OwnerAcquireOutcomeUnknown.code().to_owned())
            ),
            _ => {
                assert_eq!(result, Err(OwnerError::ContractExhausted.code().to_owned()));
                assert!(sink.requests.is_empty());
            }
        }
        flag.store(false, Ordering::Release);
        assert!(matches!(
            admit_existing(invocation()).unwrap(),
            NativeRootAdmission::Inspection(_)
        ));
        assert_eq!(
            std::fs::read(&catalog).unwrap(),
            b"# ELIOT Search source roots v1\n"
        );
    }
    // Retain native root/credential evidence; qualification owns cleanup.
}
