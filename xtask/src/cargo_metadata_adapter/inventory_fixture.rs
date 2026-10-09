use super::*;

#[test]
fn official_commands_are_locked_offline_with_separate_inventory_scope() {
    for no_deps in [false, true] {
        let command = metadata_command(Path::new("fixture"), no_deps);
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().expect("fixture UTF-8"))
            .collect();
        assert!(args.contains(&"metadata"));
        assert!(args.contains(&"--locked"));
        assert!(args.contains(&"--offline"));
        assert_eq!(args.contains(&"--no-deps"), no_deps);
        assert_eq!(args.contains(&"--all-features"), !no_deps);
        assert!(!args.contains(&"--filter-platform"));
    }
}

#[test]
fn bounded_display_checks_before_growth_and_preserves_utf8() {
    let exact = "é".repeat(MAX_TEXT_BYTES / 2);
    assert_eq!(display(&exact).expect("exact bound"), exact);
    assert!(display(&format!("{exact}x")).is_err());
    let mut sink = BoundedText(exact.clone());
    assert!(std::fmt::Write::write_str(&mut sink, "x").is_err());
    assert_eq!(sink.0, exact);
}

#[test]
fn successful_transport_with_invalid_json_is_terminal() {
    assert!(
        parse_output(&CapturedOutput {
            stdout: b"not metadata".to_vec(),
            stderr: Vec::new()
        })
        .is_err()
    );
}

#[test]
fn locked_metadata_failure_preserves_input_and_never_creates_lockfile() {
    let root = std::env::temp_dir().join(format!("eliot-locked-metadata-{}", std::process::id()));
    std::fs::create_dir(&root).expect("unique native fixture root");
    let manifest = b"[package]\nname='locked-metadata-fixture'\nversion='0.0.0'\nedition='2024'\n[dependencies]\ncargo_metadata='=0.23.1'\n";
    std::fs::write(root.join("Cargo.toml"), manifest).expect("fixture manifest");
    std::fs::create_dir(root.join("src")).expect("fixture src");
    std::fs::write(root.join("src/lib.rs"), b"").expect("fixture library");
    let result = load_inventory(&root, false);
    let unchanged = std::fs::read(root.join("Cargo.toml")).expect("read fixture");
    let lock_exists = root.join("Cargo.lock").exists();
    std::fs::remove_dir_all(&root).expect("remove exact fixture root");
    assert!(result.is_err(), "unlocked fixture must not become a graph");
    assert_eq!(unchanged, manifest);
    assert!(!lock_exists);
}
