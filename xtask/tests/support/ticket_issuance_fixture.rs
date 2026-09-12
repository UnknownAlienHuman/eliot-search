use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use xtask::ticket_planner::CONTROL_ROOTS;

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

    pub fn add_accepted_contracts_handoff(&self, valid_signature: bool) -> (String, String) {
        let accepted_commit = self.tagged_head();
        let prefix = format!(
            concat!(
                "schema_version = 1\n",
                "record_kind = \"package_handoff_v1\"\n",
                "status = \"ACCEPTED\"\n\n",
                "[identity]\n",
                "handoff_id = \"contracts-001\"\n",
                "operation_id = \"{}\"\n",
                "package = \"search-contracts\"\n",
                "stage = \"W0\"\n",
                "accepted_at = \"2026-08-31T00:00:00Z\"\n\n",
                "[accepted_code]\n",
                "base_commit = \"{}\"\n",
                "final_commit = \"{}\"\n",
                "changed_files_digest = \"{}\"\n\n",
                "[public_surface]\n",
                "api_manifest_ref = \"artifact:contracts-api\"\n",
                "api_schema_digest = \"{}\"\n",
                "configuration_digest = \"ABSENT\"\n",
                "fixture_digest_set = []\n",
                "error_reason_digest = \"{}\"\n\n",
            ),
            "3".repeat(64),
            accepted_commit,
            accepted_commit,
            "4".repeat(64),
            "1".repeat(64),
            "2".repeat(64),
        );
        let digest = if valid_signature {
            format!("{:x}", Sha256::digest(prefix.as_bytes()))
        } else {
            "0".repeat(64)
        };
        let text = format!(
            concat!(
                "{}",
                "[signature]\n",
                "record_sha256 = \"{}\"\n",
                "integration_signature_ref = \"signature:contracts\"\n",
            ),
            prefix, digest,
        );
        let path = "swarm/handoffs/search-contracts/contracts-001.toml";
        self.write_text(path, &text);
        (path.to_owned(), self.commit("accepted contracts handoff"))
    }

    fn git(&self, args: &[&str]) -> String {
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

    fn git_input(&self, args: &[&str], input: &[u8]) -> String {
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

    fn ticket(package: &str, launch_class: &str) -> String {
        let conditional = package != "search-contracts";
        let dependency_fields = if conditional {
            concat!(
                "required_handoff_packages = [\"search-contracts\"]\n",
                "accepted_handoff_refs = []\n",
                "required_contract_commit = \"UNSELECTED\"\n",
                "required_contract_api_schema_digest = \"UNAVAILABLE\"\n",
                "status = \"UNAVAILABLE\"",
            )
        } else {
            concat!(
                "required_handoff_packages = []\n",
                "accepted_handoff_refs = []\n",
                "status = \"NOT_REQUIRED\"",
            )
        };
        let soft = match package {
            "search-contracts" => 8000,
            "search-domain" => 7000,
  ²È="25Á…Ñ¡Ì€ôl‰‘½Ì½…É¡¥Ñ•ÑÕÉ”¼¨¨ˆ°€‰‰¥¹Ì¼¨¨‰t)É•ÅÕ¥É•‘}Õ¹…Ù…¥±…‰±•}¡•­Ì€ôl‰É•…±}Ñ½½±¡…¥¸‰t(ˆŒ°(€€€€€€€€€€€Í½ÕÉ•}½Õ¹Ð€ôÍ½ÕÉ•Ì¹±•¸ ¤°(€€€€€€€€€€€Í•±•Ñ½É}½Õ¹Ð€ôÍ•±•Ñ½ÉÌ¹±•¸ ¤°(€€€€€€€€€€€Í±½Ñ}½Õ¹Ð€ô¥˜½¹‘¥Ñ¥½¹…°ì€Äô•±Í”ì€Àô°(€€€€€€€€¤(€€€ô((€€€™¸ÝÉ¥Ñ•}™¥áÑÕÉ” ™Í•±˜¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰9QL¹µˆ°€ˆŒÉ½½Ñq¸ˆ¤ì(€€€€€€€™½ÈÁ…­…”¥¸l‰Í•…É µ½¹ÑÉ…ÑÌˆ°€‰Í•…É µ‘½µ…¥¸ˆ°€‰Í•…É µÁ½ÉÑÌ‰tì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ‰É…Ñ•Ì½íÁ…­…•ô½9QL¹µˆ¤°(€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ˆŒíÁ…­…•õq¸ˆ¤°(€€€€€€€€€€€€¤ì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½íÁ…­…•ô¹µˆ¤°(€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ˆŒíÁ…­…•ô…ÍÍ¥¹µ•¹Ñq¸ˆ¤°(€€€€€€€€€€€€¤ì(€€€€€€€ô(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰‘½Ì½¡…¹‘½™˜½UQ!=I%Qe}5@¹µˆ°€ˆŒ…ÕÑ¡½É¥Ñåq¸ˆ¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰‘½Ì½¡…¹‘½™˜½@ÀÁ}	==QMQI@¹µˆ°€ˆŒ‰½½ÑÍÑÉ…Áq¸ˆ¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰ÍÝ…É´½MM%959Q}AI=Q==0¹µˆ°€ˆŒ…ÍÍ¥¹µ•¹ÐÁÉ½Ñ½½±q¸ˆ¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰‘½Ì½½¹ÑÉ…ÑÌ½ÀÀÀ½µ…¹¥™•ÍÐ¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Åq¸ˆ°(€€€€€€€€€€€€€€€€‰É•ÅÕ¥É•‘}™¥±•Ì€ômp‰I5¹µ‘pˆ°p‰9=9%1}QeAL¹µ‘pˆ°p‰QeA}I%MQId¹µ‘p‰uq¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€™½È¹…µ”¥¸l‰I5¹µˆ°€‰9=9%1}QeAL¹µˆ°€‰QeA}I%MQId¹µ‰tì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ‰‘½Ì½½¹ÑÉ…ÑÌ½ÀÀÀ½í¹…µ•ôˆ¤°(€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ˆŒí¹…µ•õq¸ˆ¤°(€€€€€€€€€€€€¤ì(€€€€€€€ô((€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½É…Ñ•Ì¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Ü)mmÁ…­…•ut)¹…µ”€ô€‰Í•…É µ½¹ÑÉ…ÑÌˆ)Á…Ñ €ô€‰É…Ñ•Ì½Í•…É µ½¹ÑÉ…ÑÌˆ)™…µ¥±ä€ô€‰™½Õ¹‘…Ñ¥½¸ˆ)Ý…Ù”€ô€À)Í½™Ñ}ÍÉ}±¥¹•}Ñ…É•Ð€ô€ÜÔÀÀ)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µ½¹ÑÉ…ÑÌ¹µˆ()mmÁ…­…•ut)¹…µ”€ô€‰Í•…É µ‘½µ…¥¸ˆ)Á…Ñ €ô€‰É…Ñ•Ì½Í•…É µ‘½µ…¥¸ˆ)™…µ¥±ä€ô€‰™½Õ¹‘…Ñ¥½¸ˆ)Ý…Ù”€ô€À)Í½™Ñ}ÍÉ}±¥¹•}Ñ…É•Ð€ô€ÜÀÀÀ)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µ‘½µ…¥¸¹µˆ()mmÁ…­…•ut)¹…µ”€ô€‰Í•…É µÁ½ÉÑÌˆ)Á…Ñ €ô€‰É…Ñ•Ì½Í•…É µÁ½ÉÑÌˆ)™…µ¥±ä€ô€‰™½Õ¹‘…Ñ¥½¸ˆ)Ý…Ù”€ô€À)Í½™Ñ}ÍÉ}±¥¹•}Ñ…É•Ð€ô€ÔÔÀÀ)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µÁ½ÉÑÌ¹µˆ(ˆŒ°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½™Õ¹Ñ¥½¸µÁ…­•ÑÌ¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Ä)mm™½Õ¹‘…Ñ¥½¹ut)Á…­…”€ô€‰Í•…É µ½¹ÑÉ…ÑÌˆ)Ý…Ù”€ô€À)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µ½¹ÑÉ…ÑÌ¹µˆ)ÝÉ¥Ñ•}Í½Á”€ô€‰É…Ñ•Ì½Í•…É µ½¹ÑÉ…ÑÌ¼¨¨ˆ()mm™½Õ¹‘…Ñ¥½¹ut)Á…­…”€ô€‰Í•…É µ‘½µ…¥¸ˆ)Ý…Ù”€ô€À)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µ‘½µ…¥¸¹µˆ)ÝÉ¥Ñ•}Í½Á”€ô€‰É…Ñ•Ì½Í•…É µ‘½µ…¥¸¼¨¨ˆ()mm™½Õ¹‘…Ñ¥½¹ut)Á…­…”€ô€‰Í•…É µÁ½ÉÑÌˆ)Ý…Ù”€ô€À)…ÍÍ¥¹µ•¹Ð€ô€‰ÍÝ…É´½…ÍÍ¥¹µ•¹ÑÌ½Í•…É µÁ½ÉÑÌ¹µˆ)ÝÉ¥Ñ•}Í½Á”€ô€‰É…Ñ•Ì½Í•…É µÁ½ÉÑÌ¼¨¨ˆ(ˆŒ°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½ÍÑ…•Ì¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Ä)mmÍÑ…•ut)¥€ô€‰\Àˆ)Ý…Ù”€ô€À)ÍÑ…ÑÕÌ€ô€‰Q%Y}A-}=91dˆ)Á…­…•Ì€ôl‰Í•…É µ½¹ÑÉ…ÑÌˆ°€‰Í•…É µ‘½µ…¥¸ˆ°€‰Í•…É µÁ½ÉÑÌ‰t()mmÍÑ…•ut)¥€ô€‰\Äˆ)Ý…Ù”€ô€Ä)ÍÑ…ÑÕÌ€ô€‰	1=-ˆ)É•ÅÕ¥É•Í}…•ÁÑ•‘}…Ñ•Ì€ôl‰À‰t)É•ÅÕ¥É•Í}…•ÁÑ•‘}É••¥ÁÑÌ€ôl‰\À‰t)Á…­…•Ì€ômt(ˆŒ°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½±…Õ¹ µÍÑ…Ñ”¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Ø)…Ñ¥Ù•}ÍÑ…”€ô€‰@ÀÀˆ)…Ñ¥Ù•}Ý…Ù”€ô€À)½É¡•ÍÑÉ…Ñ¥½¹}É•¥ÍÑÉå}Í¡•µ…}Ù•ÉÍ¥½¸€ô€Ô)½É¡•ÍÑÉ…Ñ¥½¹}É•¥ÍÑÉå}Á…Ñ €ô€‰ÍÝ…É´½½É¡•ÍÑÉ…Ñ¥½¸¹Ñ½µ°ˆ)…ÕÑ¡½É¥é•‘}Á…­…•Ì€ôl‰Í•…É µ½¹ÑÉ…ÑÌ‰t)½¹‘¥Ñ¥½¹…±}Á…­…•Ì€ôl‰Í•…É µ‘½µ…¥¸ˆ°€‰Í•…É µÁ½ÉÑÌ‰t()m½¹‘¥Ñ¥½¹…±}…Ñ¥Ù…Ñ¥½¸¹Í•…É µ‘½µ…¥¹t)É•ÅÕ¥É•Ì€ôl‰…•ÁÑ•½¹ÑÉ…ÑÌ¡…¹‘½™˜‰t()m½¹‘¥Ñ¥½¹…±}…Ñ¥Ù…Ñ¥½¸¹Í•…É µÁ½ÉÑÍt)É•ÅÕ¥É•Ì€ôl‰…•ÁÑ•½¹ÑÉ…ÑÌ¡…¹‘½™˜‰t(ˆŒ°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½½É¡•ÍÑÉ…Ñ¥½¸¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Õq¸ˆ°(€€€€€€€€€€€€€€€€‰Ý½É­™±½Ý}Á½±¥ä€ôp‰µ…¹Õ…±}½¹±åp‰q¸ˆ°(€€€€€€€€€€€€€€€€‰½¹ÍÕµ•É}ÕÍ•Í}‰É…¹¡}¡•…€ô™…±Í•q¸ˆ°(€€€€€€€€€€€€€€€€‰½¹ÍÕµ•É}É•ÅÕ¥É•Í}•á…Ñ}½µµ¥Ñ}…¹‘}…Á¥}‘¥•ÍÐ€ôÑÉÕ•q¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰ÍÝ…É´½½¹ÑÉ½°µÁ±…¹”µÍ¡•µ„¹Ñ½µ°ˆ°€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Íq¸ˆ¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ‰ÍÝ…É´½Í¡•µ…Ì½ÑåÁ•ÌµØÄ¹Ñ½µ°ˆ°€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Éq¸ˆ¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½Ñ¥­•Ðµ¥ÍÍÕ…¹”µÁ±…¸µÍ¡•µ„µØÈ¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Éq¸ˆ°(€€€€€€€€€€€€€€€€‰É•½É‘}­¥¹€ôp‰Ñ¥­•Ñ}¥ÍÍÕ…¹•}Á±…¹}ØÉp‰q¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½Ñ¥­•Ðµ¥ÍÍÕ…¹”µÁ±…¸µ‘¥•ÍÐµØÈ¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Éq¸ˆ°(€€€€€€€€€€€€€€€€‰Í•±™}É•™•É•¹Ñ¥…±}‘¥•ÍÑ}…±±½Ý•€ô™…±Í•q¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½Ñ¥­•Ðµ¥ÍÍÕ…¹”µÁ±…¹¹•ÈµØÈ¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Éq¸ˆ°(€€€€€€€€€€€€€€€€‰½µÁ½¹•¹Ð€ôp‰Ñ¥­•Ñ}¥ÍÍÕ…¹•}Á±…¹¹•É}ØÉp‰q¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½ÀÀÀµ™½Õ¹‘…Ñ¥½¸µ…•ÁÑ…¹”¹Ñ½µ°ˆ°(€€€€€€€€€€€½¹…Ð„ (€€€€€€€€€€€€€€€€‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€Åq¸ˆ°(€€€€€€€€€€€€€€€€‰ÍÑ…ÑÕÌ€ôp‰M%9}9=Q}aUQp‰q¸ˆ°(€€€€€€€€€€€€¤°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½Ñ¥­•Ðµ‘É…™ÑÌ½µ…¹¥™•ÍÐ¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€È)Ñ¥­•Ñ}‘É…™Ñ}Í¡•µ…}Ù•ÉÍ¥½¸€ô€È)‘É…™Ñ}½Õ¹Ð€ô€Ì)mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µ½¹ÑÉ…ÑÌˆ)Á…Ñ €ô€‰ÍÝ…É´½Ñ¥­•Ðµ‘É…™ÑÌ½ÀÀÀ½Í•…É µ½¹ÑÉ…ÑÌ¹Ñ½µ°ˆ)mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µ‘½µ…¥¸ˆ)Á…Ñ €ô€‰ÍÝ…É´½Ñ¥­•Ðµ‘É…™ÑÌ½ÀÀÀ½Í•…É µ‘½µ…¥¸¹Ñ½µ°ˆ)mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µÁ½ÉÑÌˆ)Á…Ñ €ô€‰ÍÝ…É´½Ñ¥­•Ðµ‘É…™ÑÌ½ÀÀÀ½Í•…É µÁ½ÉÑÌ¹Ñ½µ°ˆ(ˆŒ°(€€€€€€€€¤ì(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€‰ÍÝ…É´½½¹Ñ•áÐµ‘É…™ÑÌ½µ…¹¥™•ÍÐ¹Ñ½µ°ˆ°(€€€€€€€€€€€ÈŒ‰Í¡•µ…}Ù•ÉÍ¥½¸€ô€È)½¹Ñ•áÑ}‘É…™Ñ}Í¡•µ…}Ù•ÉÍ¥½¸€ô€È)‘É…™Ñ}½Õ¹Ð€ô€Ì)½É‘¥¹…Éå}ÍÑ…Ñ¥}Í½ÕÉ•}™¥±•}•¥±¥¹œ€ô€ÄØ)ÀÀÁ}•á…Ñ}½¹ÑÉ…Ñ}Á…­}Í½ÕÉ•}™¥±•}•¥±¥¹œ€ô€ÈÐ)ÀÀÁ}•á…Ñ}½¹ÑÉ…Ñ}Á…­}•á•ÁÑ¥½¹}Á…­…•Ì€ôl‰Í•…É µ½¹ÑÉ…ÑÌ‰t)µ…á}É•¥ÍÑÉå}™É…µ•¹ÑÍ}Á•É}½¹Ñ•áÐ€ô€Ø)µ…á}…•ÁÑ•‘}¡…¹‘½™™}Í±½ÑÍ}Á•É}½¹Ñ•áÐ€ô€Ä()mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µ½¹ÑÉ…ÑÌˆ)Á…Ñ €ô€‰ÍÝ…É´½½¹Ñ•áÐµ‘É…™ÑÌ½ÀÀÀ½Í•…É µ½¹ÑÉ…ÑÌ¹Ñ½µ°ˆ)Í½ÕÉ•}•¥±¥¹}±…ÍÌ€ô€‰@ÀÁ}aQ}=9QIQ}A,ˆ()mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µ‘½µ…¥¸ˆ)Á…Ñ €ô€‰ÍÝ…É´½½¹Ñ•áÐµ‘É…™ÑÌ½ÀÀÀ½Í•…É µ‘½µ…¥¸¹Ñ½µ°ˆ)Í½ÕÉ•}•¥±¥¹}±…ÍÌ€ô€‰=I%9Idˆ()mm‘É…™Ñut)Á…­…”€ô€‰Í•…É µÁ½ÉÑÌˆ)Á…Ñ €ô€‰ÍÝ…É´½½¹Ñ•áÐµ‘É…™ÑÌ½ÀÀÀ½Í•…É µÁ½ÉÑÌ¹Ñ½µ°ˆ)Í½ÕÉ•}•¥±¥¹}±…ÍÌ€ô€‰=I%9Idˆ(ˆŒ°(€€€€€€€€¤ì(€€€€€€€™½È€¡Á…­…”°±…Õ¹¡}±…ÍÌ¤¥¸l(€€€€€€€€€€€€ ‰Í•…É µ½¹ÑÉ…ÑÌˆ°€‰UQ!=I%iˆ¤°(€€€€€€€€€€€€ ‰Í•…É µ‘½µ…¥¸ˆ°€‰=9%Q%=90ˆ¤°(€€€€€€€€€€€€ ‰Í•…É µÁ½ÉÑÌˆ°€‰=9%Q%=90ˆ¤°(€€€€€€€tì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ‰ÍÝ…É´½Ñ¥­•Ðµ‘É…™ÑÌ½ÀÀÀ½íÁ…­…•ô¹Ñ½µ°ˆ¤°(€€€€€€€€€€€€€€€€™M•±˜èéÑ¥­•Ð¡Á…­…”°±…Õ¹¡}±…ÍÌ¤°(€€€€€€€€€€€€¤ì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€€€€€™™½Éµ…Ð„ ‰ÍÝ…É´½½¹Ñ•áÐµ‘É…™ÑÌ½ÀÀÀ½íÁ…­…•ô¹Ñ½µ°ˆ¤°(€€€€€€€€€€€€€€€€™M•±˜èé½¹Ñ•áÐ¡Á…­…”¤°(€€€€€€€€€€€€¤ì(€€€€€€€ô(€€€€€€€™½ÈÉ½½Ð¥¸=9QI=1}I==QLì(€€€€€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ ™™½Éµ…Ð„ ‰íÉ½½Ñô½I5¹µˆ¤°€ˆŒÉ•Í•ÉÙ•‘q¸ˆ¤ì(€€€€€€€ô(€€€€€€€Í•±˜¹ÝÉ¥Ñ•}Ñ•áÐ (€€€€€€€€€€€€ˆ¹¥Ñ¡Õˆ½Ý½É­™±½ÝÌ½µ…¹Õ…°¹åµ°ˆ°(€€€€€€€€€€€ÈŒ‰¹…µ”è5…¹Õ…°)½¸è(€Ý½É­™±½Ý}‘¥ÍÁ…Ñ è)Á•Éµ¥ÍÍ¥½¹Ìè(€½¹Ñ•¹ÑÌèÉ•…)©½‰Ìè(€Ù…±¥‘…Ñ”è(€€€ÉÕ¹Ìµ½¸èÝ¥¹‘½ÝÌµ±…Ñ•ÍÐ(€€€ÍÑ•ÁÌè(€€€€€€´ÕÍ•Ìè…Ñ¥½¹Ì½¡•­½ÕÑ ÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀÀ(€€€€€€€Ý¥Ñ è(€€€€€€€€€Á•ÉÍ¥ÍÐµÉ•‘•¹Ñ¥…±Ìè™…±Í”(ˆŒ°(€€€€€€€€¤ì(€€€ô)ô()¥µÁ°É½À™½È¥áÑÕÉ•I•Á½Í¥Ñ½Éäì(€€€™¸‘É½À ™µÕÐÍ•±˜¤ì(€€€€€€€±•Ð|€ô™ÌèéÉ•µ½Ù•}‘¥É}…±° ™Í•±˜¹É½½Ð¤ì(€€€ô)ô()™¸É•¹‘•É}…ÉÉ…å}±¥¹•Ì¡Ù…±Õ•Ìè€™mMÑÉ¥¹t¤€´øMÑÉ¥¹œì(€€€Ù…±Õ•Ì(€€€€€€€€¹¥Ñ•È ¤(€€€€€€€€¹µ…À¡ñÙ…±Õ•ð™½Éµ…Ð„ ˆ€íôˆ°Í•É‘•}©Í½¸èéÑ½}ÍÑÉ¥¹œ¡Ù…±Õ”¤¹•áÁ•Ð ‰)M=8ÍÑÉ¥¹œˆ¤¤¤(€€€€€€€€¹½±±•ÐèèñY•Œñ|øø ¤(€€€€€€€€¹©½¥¸ ˆ±q¸ˆ¤)ô(