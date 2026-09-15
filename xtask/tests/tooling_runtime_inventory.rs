//! Repository-wide T41 guard for executable tooling roots.
//!
//! Explicit wrapper mappings remain covered by `tooling_runtime_boundary`;
//! this companion test rejects restoration of Python/Node source, manifests or
//! executable commands anywhere under the required tooling roots.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_EXTENSIONS: &[&str] =
    &["py", "pyw", "js", "mjs", "cjs", "ts", "tsx"];
const FORBIDDEN_MANIFESTS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "pyproject.toml",
    "requirements.txt",
    "pipfile",
    "poetry.lock",
];
const FORBIDDEN_COMMANDS: &[&str] = &[
    "python",
    "python3",
    "python.exe",
    "python3.exe",
    "py",
    "py.exe",
    "node",
    "node.exe",
    "npx",
    "npx.cmd",
    "npm",
    "npm.cmd",
    "pnpm",
    "pnpm.cmd",
    "yarn",
    "yarn.cmd",
];

#[test]
fn required_tooling_roots_are_rust_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let mut files = Vec::new();
    for relative in ["tools", ".github/workflows"] {
        collect_files(&root.join(relative), &mut files);
    }

    for path in files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        assert!(
            !FORBIDDEN_EXTENSIONS.contains(&extension.as_str()),
            "required tooling restored a Python/Node-family source: {}",
            path.display()
        );
        assert!(
            !FORBIDDEN_MANIFESTS.contains(&name.as_str()),
            "required tooling restored a Python/Node runtime manifest: {}",
            path.display()
        );

        if matches!(extension.as_str(), "ps1" | "yml" | "yaml") {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            for (index, line) in text.lines().enumerate() {
                if let Some(runtime) = forbidden_runtime_invocation(line) {
                    panic!(
                        "{}:{} invokes forbidden required runtime {runtime}",
                        path.display(),
                        index + 1
                    );
                }
            }
        }
    }
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            !file_type.is_symlink(),
            "required tooling roots must not contain symbolic links: {}",
            path.display()
        );
        if file_type.is_dir() {
            collect_files(&path, files);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}

fn forbidden_runtime_invocation(line: &str) -> Option<&'static str> {
    for segment in line.split([';', '|', '{', '}']) {
        let mut command = segment.trim();
        if command.is_empty() || command.starts_with('#') {
            continue;
        }
        if let Some(rest) = command.strip_prefix("- ") {
            command = rest.trim_start();
        }
        if let Some(rest) = command.strip_prefix("run:") {
            command = rest.trim_start();
        }
        for prefix in ["& ", ". ", "call ", "cmd /c ", "cmd.exe /c "] {
            if command
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            {
                command = command[prefix.len()..].trim_start();
                break;
            }
        }

        let tokens = command
            .split_whitespace()
            .map(normalize_command_token)
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        let Some(head) = tokens.first() else {
            continue;
        };
        if let Some(runtime) = forbidden_command(head) {
            return Some(runtime);
        }

        if matches!(
            head.as_str(),
            "start-process"
                | "invoke-expression"
                | "bash"
                | "sh"
                | "cmd"
                | "cmd.exe"
                | "pwsh"
                | "powershell"
                | "powershell.exe"
        ) {
            if let Some(runtime) = tokens
                .iter()
                .skip(1)
                .find_map(|token| forbidden_command(token))
            {
                return Some(runtime);
            }
        }
    }
    None
}

fn normalize_command_token(token: &str) -> String {
    token
        .trim_matches(|character: char| {
            matches!(
                character,
                '\'' | '"' | '`' | '(' | ')' | '[' | ']' | ',' | ':'
            )
        })
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn forbidden_command(token: &str) -> Option<&'static str> {
    FORBIDDEN_COMMANDS
        .iter()
        .copied()
        .find(|forbidden| token == *forbidden)
}
