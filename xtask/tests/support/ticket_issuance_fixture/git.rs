use std::io::Write;
use std::process::{Command, Stdio};

use super::FixtureRepository;

impl FixtureRepository {
    pub fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "--quiet", "-m", message]);
        self.tagged_head()
    }

    pub fn tagged_head(&self) -> String {
        let algorithm = self.git(&["rev-parse", "--show-object-format"]);
        let head = self.git(&["rev-parse", "HEAD"]);
        format!("{algorithm}:{head}")
    }

    pub fn commit_index_symlink(&self, relative: &str, target: &str) -> String {
        let blob = self.git_input(&["hash-object", "-w", "--stdin"], target.as_bytes());
        let cache_info = format!("120000,{blob},{relative}");
        self.git(&["update-index", "--add", "--cacheinfo", &cache_info]);
        self.git(&["commit", "--quiet", "-m", "symlink source"]);
        self.tagged_head()
    }

    pub(super) fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .expect("execute git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("git stdout is UTF-8")
            .trim()
            .to_owned()
    }

    pub(super) fn git_input(&self, args: &[&str], input: &[u8]) -> String {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn git");
        child
            .stdin
            .take()
            .expect("git stdin")
            .write_all(input)
            .expect("write git stdin");
        let output = child.wait_with_output().expect("wait for git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("git stdout is UTF-8")
            .trim()
            .to_owned()
    }
}
