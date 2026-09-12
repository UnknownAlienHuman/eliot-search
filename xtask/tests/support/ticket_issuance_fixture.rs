mod git;
mod handoff;
mod repository;
mod templates;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub struct FixtureRepository {
    root: PathBuf,
}

impl FixtureRepository {
    pub fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-ticket-plan-{}-{stamp}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).expect("create fixture root");
        let fixture = Self { root };
        fixture.git(&["init", "--quiet"]);
        fixture.git(&["config", "user.email", "planner@example.invalid"]);
        fixture.git(&["config", "user.name", "Planner Tests"]);
        fixture.write_fixture();
        fixture.commit("fixture");
        fixture
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_text(&self, relative: &str, text: &str) {
        self.write_bytes(relative, text.as_bytes());
    }

    pub fn write_bytes(&self, relative: &str, bytes: &[u8]) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, bytes).expect("write fixture file");
    }

    pub fn read_text(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("read fixture file")
    }

    pub fn replace_once(&self, relative: &str, old: &str, new: &str) {
        let text = self.read_text(relative);
        assert_eq!(
            text.matches(old).count(),
            1,
            "{relative}: expected exactly one {old:?}"
        );
        self.write_text(relative, &text.replacen(old, new, 1));
    }

    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.root.join(relative)).expect("remove fixture file");
    }

    pub fn append_text(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open fixture append target");
        file.write_all(text.as_bytes()).expect("append fixture text");
    }
}

impl Drop for FixtureRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
