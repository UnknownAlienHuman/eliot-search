//! A real Windows descendant retains both daemon pipes after its wrapper exits.

#![cfg(windows)]

use std::fs;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CLI_DEADLINE: Duration = Duration::from_secs(120);
const SCHEDULING_ALLOWANCE: Duration = Duration::from_secs(5);

struct Sandbox {
    base: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-cli-pipe-deadline-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&base).expect("create process-test directory");
        Self {
            base: std::path::absolute(base).expect("absolute process-test directory"),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Ok(text) = fs::read_to_string(self.base.join("holder.pid"))
            && let Ok(pid) = text.parse::<u32>()
        {
            let _ = Command::new("taskkill.exe")
                .args(["/PID", &pid.to_string(), "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn wait_for_exit(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("observe CLI process: {error}");
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_capture(reader: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    reader
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .expect("read completed CLI capture");
    assert!(
        bytes.len() <= 16 * 1024,
        "CLI capture exceeded its test bound"
    );
    bytes
}

#[test]
fn descendant_pipes_cannot_extend_the_original_cli_deadline() {
    let sandbox = Sandbox::new();
    let wrapper = sandbox.base.join("wrapper.cmd");
    let wrapper_script = sandbox.base.join("wrapper.ps1");
    let holder_script = sandbox.base.join("holder.ps1");
    let holder_ready = sandbox.base.join("holder.ready");
    let wrapper_exited = sandbox.base.join("wrapper.exited");
    fs::write(
        &wrapper,
        concat!(
            "@echo off\r\n",
            "\"%ELIOT_PIPE_TEST_SHELL%\" -NoLogo -NoProfile -NonInteractive ",
            "-ExecutionPolicy Bypass -File \"%ELIOT_PIPE_TEST_WRAPPER%\"\r\n",
            "if errorlevel 1 exit /b %errorlevel%\r\n",
            "type nul > \"%ELIOT_PIPE_TEST_EXITED%\"\r\n",
            "exit /b 0\r\n",
        ),
    )
    .expect("write native wrapper");
    fs::write(
        &wrapper_script,
        r#"$ErrorActionPreference = 'Stop'
$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $env:ELIOT_PIPE_TEST_SHELL
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
# Redirecting stdin makes .NET explicitly pass the inherited stdout/stderr.
$start.RedirectStandardInput = $true
$start.Arguments = '-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $env:ELIOT_PIPE_TEST_HOLDER + '"'
$holder = [Diagnostics.Process]::Start($start)
[IO.File]::WriteAllText($env:ELIOT_PIPE_TEST_PID, [string]$holder.Id)
$holder.StandardInput.Close()
$readyDeadline = [DateTime]::UtcNow.AddSeconds(10)
while (-not [IO.File]::Exists($env:ELIOT_PIPE_TEST_READY)) {
    if ($holder.HasExited -or [DateTime]::UtcNow -ge $readyDeadline) { exit 3 }
    [Threading.Thread]::Sleep(10)
}
"#,
    )
    .expect("write wrapper process");
    fs::write(
        &holder_script,
        r"$ErrorActionPreference = 'Stop'
$heldOut = [Console]::OpenStandardOutput()
$heldErr = [Console]::OpenStandardError()
$bytes = [Text.Encoding]::ASCII.GetBytes('pipe held')
$heldOut.Write($bytes, 0, $bytes.Length)
$heldOut.Flush()
$heldErr.Write($bytes, 0, $bytes.Length)
$heldErr.Flush()
[IO.File]::WriteAllText($env:ELIOT_PIPE_TEST_READY, 'ready')
[Threading.Thread]::Sleep(180000)
",
    )
    .expect("write descendant process");
    let shell = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system root"))
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let started = Instant::now();
    let mut cli = Command::new(env!("CARGO_BIN_EXE_eliot-search"))
        .arg("inspect-data-root")
        .arg(sandbox.base.join("unused-root"))
        .arg("--daemon")
        .arg(&wrapper)
        .env("ELIOT_PIPE_TEST_SHELL", shell)
        .env("ELIOT_PIPE_TEST_WRAPPER", wrapper_script)
        .env("ELIOT_PIPE_TEST_HOLDER", holder_script)
        .env("ELIOT_PIPE_TEST_PID", sandbox.base.join("holder.pid"))
        .env("ELIOT_PIPE_TEST_READY", &holder_ready)
        .env("ELIOT_PIPE_TEST_EXITED", &wrapper_exited)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .expect("start actual CLI with native wrapper");
    let status = wait_for_exit(&mut cli, started + CLI_DEADLINE + SCHEDULING_ALLOWANCE);
    let elapsed = started.elapsed();
    if status.is_none() {
        let _ = cli.kill();
        cli.wait().expect("reap CLI that exceeded its deadline");
    }
    let stdout = read_capture(cli.stdout.take().expect("CLI stdout"));
    let stderr = read_capture(cli.stderr.take().expect("CLI stderr"));

    assert!(holder_ready.is_file(), "descendant pipes unavailable");
    assert!(wrapper_exited.is_file(), "wrapper did not finish its work");
    let status = status.expect("CLI waited past its original deadline for descendant pipes");
    assert_eq!(
        status.code(),
        Some(124),
        "unexpected CLI result: {stderr:?}"
    );
    assert!(stdout.is_empty(), "incomplete stdout escaped the CLI");
    assert_eq!(stderr, b"{\"error\":\"DAEMON_TIMEOUT\"}\n");
    assert!(
        elapsed
            >= CLI_DEADLINE
                .checked_sub(Duration::from_secs(1))
                .expect("deadline exceeds scheduling tolerance")
    );
    assert!(elapsed <= CLI_DEADLINE + SCHEDULING_ALLOWANCE);
}
