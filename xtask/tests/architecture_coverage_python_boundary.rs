use std::path::{Path, PathBuf};

const RETIRED_DUPLICATE: [&str; 6] = [
    "tools/architecture_coverage_validator_v1/__init__.py",
    "tools/architecture_coverage_validator_v1/common.py",
    "tools/architecture_coverage_validator_v1/control.py",
    "tools/architecture_coverage_validator_v1/run.py",
    "tools/architecture_coverage_validator_v1/schemas.py",
    "tools/architecture_coverage_validator_v1/topology.py",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

#[test]
fn stale_architecture_python_duplicate_stays_retired() {
    let root = repository_root();
    for relative in RETIRED_DUPLICATE {
        assert!(
            !root.join(relative).exists(),
            "retired duplicate returned: {relative}"
        );
    }

    for directory in ["tools", ".github/workflows", "qualification", "docs"] {
        let root_dir = root.join(directory);
        let mut pending = vec![root_dir];
        while let Some(current) = pending.pop() {
            for entry in std::fs::read_dir(&current)
                .unwrap_or_else(|error| panic!("{}: {error}", current.display()))
            {
                let entry = entry.expect("repository entry must be readable");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                assert!(
                    !text.contains("architecture_coverage_validator_v1"),
                    "{} references the retired duplicate package",
                    path.display()
                );
            }
        }
    }
}
