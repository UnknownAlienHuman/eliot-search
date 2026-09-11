//! Prevents the implementation-program validator from becoming one large file again.

use std::fs;
use std::path::{Path, PathBuf};

const VALIDATOR: &str = "xtask/src/impl_program.rs";
const MODULE_ROOT: &str = "xtask/src/impl_program";
const MAX_FACADE_LINES: usize = 100;
const SPLIT_REVIEW_LINES: usize = 8_500;
const MODULES: [(&str, &[&str]); 4] = [
    ("model", &["pub struct ProgramReport", "pub const EXPECTED_STAGE_IDS"]),
    ("parse", &["fn load_doc", "fn rows", "fn index_all"]),
    ("report", &["pub const fn exit_code", "pub fn render_report_json"]),
    ("rules", &["pub fn validate_implementation_program"]),
];
const RULE_MODULES: [(&str, &[&str]); 3] = [
    ("program", &["fn check_paths", "fn check_current_state"]),
    ("stages", &["fn check_stage_order", "fn check_targets"]),
    ("quality", &["fn check_slo", "fn check_workflow"]),
];

#[test]
fn validator_is_a_bounded_facade_over_responsibility_modules() {
    let root = workspace_root();
    let facade_path = root.join(VALIDATOR);
    let facade = read(&facade_path);

    assert!(
        facade.lines().count() <= MAX_FACADE_LINES,
        "{} must remain a bounded facade",
        facade_path.display()
    );
    for forbidden in ["fn load_doc", "fn append_json_string", "fn check_paths"] {
        assert!(
            !facade.contains(forbidden),
            "validator facade regained implementation token: {forbidden}"
        );
    }

    for (module, owned_tokens) in MODULES {
        assert!(
            facade.contains(&format!("mod {module};")),
            "validator facade lacks module declaration: {module}"
        );
        assert_module(root.join(MODULE_ROOT).join(format!("{module}.rs")), owned_tokens);
    }

    let rules_path = root.join(MODULE_ROOT).join("rules.rs");
    let rules = read(&rules_path);
    for (module, owned_tokens) in RULE_MODULES {
        assert!(
            rules.contains(&format!("mod {module};")),
            "rules facade lacks module declaration: {module}"
        );
        assert_module(
            root.join(MODULE_ROOT)
                .join("rules")
                .join(format!("{module}.rs")),
            owned_tokens,
        );
    }

    for public_surface in [
        "pub use model::",
        "pub use report::{exit_code, render_report_json};",
        "pub use rules::validate_implementation_program;",
    ] {
        assert!(
            facade.contains(public_surface),
            "validator facade lost public surface: {public_surface}"
        );
    }
}

fn assert_module(path: PathBuf, owned_tokens: &[&str]) {
    let source = read(&path);
    let lines = source.lines().count();
    assert!(
        lines < SPLIT_REVIEW_LINES,
        "{} exceeds the {}-line split-review boundary: {lines}",
        path.display(),
        SPLIT_REVIEW_LINES
    );
    for token in owned_tokens {
        assert!(
            source.contains(token),
            "{} does not own expected token: {token}",
            path.display()
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member")
        .to_owned()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
