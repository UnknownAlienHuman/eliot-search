use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn is_textual_utf8(bytes: &[u8]) -> bool {
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return false;
    }
    let sample = bytes.iter().take(32 * 1024);
    let mut controls = 0_usize;
    let mut observed = 0_usize;
    for byte in sample {
        observed = observed.saturating_add(1);
        if *byte < 0x20 && !matches!(*byte, b'\t' | b'\n' | b'\r' | 0x0c) {
            controls = controls.saturating_add(1);
        }
    }
    controls <= observed.div_ceil(100).max(4)
}

pub(super) fn should_skip_directory(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git"
            | ".hg"
            | ".svn"
            | ".eliot-search"
            | ".cache"
            | ".tox"
            | ".venv"
            | "venv"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "vendor"
            | "__pycache__"
            | ".ssh"
            | ".gnupg"
    )
}

pub(super) fn policy_denies_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    let lower = name.to_ascii_lowercase();
    if lower == ".env"
        || lower.starts_with(".env.")
        || matches!(
            lower.as_str(),
            "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | "wallet.dat"
        )
    {
        return true;
    }
    let denied_extensions = [
        "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib", "class",
        "jar", "war", "zip", "7z", "rar", "gz", "bz2", "xz", "tar", "pdf",
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "mp3", "wav", "flac",
        "mp4", "mkv", "mov", "avi", "db", "sqlite", "sqlite3", "mdb", "pdb",
        "key", "pem", "pfx", "p12", "jks", "keystore", "kdbx", "der", "crt",
    ];
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            denied_extensions.contains(&extension.to_ascii_lowercase().as_str())
        })
}

pub(super) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(super) fn system_time_nanos(value: Option<SystemTime>) -> Option<u128> {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}
