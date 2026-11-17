//! A real Windows descendant retains both daemon pipes after its wrapper exits.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{ChildFailure, MAX_CAPTURE_BYTES, capture_command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TEST_DEADLINE: Duration = Duration::from_secs(3);
const SCHEDULING_ALLOWANCE: Duration = Duration::from_secs(3);

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
        let _ = fs::write(self.base.join("holder.stop"), b"stop");
        if self.base.join("holder.ready").is_file() {
            let done = self.base.join("holder.done");
            let deadline = Instant::now() + Duration::from_secs(2);
            while !done.is_file() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(20));
            }
        }
        let _ = fs::remove_dir_all(&self.base);
    }
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
$holdDeadline = [DateTime]::UtcNow.AddSeconds(15)
while (-not [IO.File]::Exists($env:ELIOT_PIPE_TEST_STOP) -and [DateTime]::UtcNow -lt $holdDeadline) {
    [Threading.Thread]::Sleep(20)
}
$heldOut.Dispose()
$heldErr.Dispose()
[IO.File]::WriteAllText($env:ELIOT_PIPE_TEST_DONE, 'done')
",
    )
    .expect("write descendant process");
    let shell = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows system root"))
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(&wrapper);
    command
        .env("ELIOT_PIPE_TEST_SHELL", shell)
        .env("ELIOT_PIPE_TEST_WRAPPER", wrapper_script)
        .env("ELIOT_PIPE_TEST_HOLDER", holder_script)
        .env("ELIOT_PIPE_TEST_STOP", sandbox.base.join("holder.stop"))
        .env("ELIOT_PIPE_TEST_DONE", sandbox.base.join("holder.done"))
        .env("ELIOT_PIPE_TEST_READY", &holder_ready)
        .env("ELIOT_PIPE_TEST_EXITED", &wrapper_exited)
        .creation_flags(CREATE_NO_WINDOW);
    let started = Instant::now();
    let output = capture_command(&mut command, MAX_CAPTURE_BYTES, started + TEST_DEADLINE);
    let elapsed = started.elapsed();

    assert!(holder_ready.is_file(), "descendant pipes unavailable");
    assert!(wrapper_exited.is_file(), "wrapper did not finish its work");
    assert!(
        !sandbox.base.join("holder.done").is_file(),
        "descendant released its pipes too early"
    );
    assert!(matches!(output, Err(ChildFailure::Timeout)));
    assert_eq!(ChildFailure::Timeout.code(), "DAEMON_TIMEOUT");
    assert_eq!(ChildFailure::Timeout.exit_code(), 124);
    assert!(elapsed >= TEST_DEADLINE);
    assert!(elapsed <= TEST_DEADLINE + SCHEDULING_ALLOWANCE);
}
