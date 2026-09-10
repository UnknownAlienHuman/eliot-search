//! T36 baseline-recipe process evidence on the DIRECT spine.
//!
//! The eleven v1 recipes are exercised through the stable primary-daemon CLI
//! surface only: no provider/query composition changes, no indexed/Qdrant
//! path (out of scope for T36 while `#125` T28 indexed stays open).
//!
//! Coverage assignment (DIRECT spine only):
//!
//! ```text
//! find_text@1               --search-root positive/denied-partial (below)
//! corpus_profile@1          --list-sources/--verify-root counts (below)
//! corpus_delta@1            re-index changed flags (below)
//! provenance@1              --read-revision exact readback (below)
//! expand_handle@1           --read-revision excerpt slice (below)
//! locate/inspect/explore/   DIRECT-backed evidence triple (below) plus the
//!   compare_implementations deterministic ladder/ambiguity/lineage library
//!   tests in search-subject-resolver and search-comparator
//! compile/execute_exact_scan exact-only, T34-proven (exact_proof_process)
//! ```
//!
//! Every test drives the real `eliot-searchd` binary. Partial and degraded
//! outcomes stay typed data (`complete=false` with explicit gaps) and are
//! never relabelled success. Unsafe code stays confined to `common` (the
//! Win32 credential-cleanup call shared by every process test); this file
//! adds none.

#[allow(dead_code)]
mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::{
    BoundedCanonicalBytes, BoundedList, CasePolicy, ExactInputDomain, ExactPredicate,
    ExactPredicateKind, FindTextRecipe, ProfileId, RecipeBodyV1, RecipeIdV1, RequestId,
    RequestedScope, SearchRecipeRequest, SourceRevisionId, SourceView,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Real workspace packages used as the corpus dictionary: every indexed
/// fixture below names actual crates, never invented text.
const CRATE_DICTIONARY: [&str; 4] = [
    "search-comparator",
    "search-query-planner",
    "search-subject-resolver",
    "search-contracts",
];

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    files: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-recipes-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let data = base.join("data");
        let files = base.join("files");
        fs::create_dir_all(&data).expect("fixture data dir");
        fs::create_dir_all(&files).expect("fixture files dir");
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self {
            base,
            data,
            files,
            guard,
        }
    }

    fn run(&self, args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("primary daemon");
        let stdout = child.stdout.take().expect("stdout pipe");
        let stderr = child.stderr.take().expect("stderr pipe");
        let out = thread::spawn(move || read_output(stdout));
        let err = thread::spawn(move || read_output(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll child") {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("primary daemon exceeded the test deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = out.join().expect("drain stdout");
        let stderr = err.join().expect("drain stderr");
        self.guard.refresh();
        (status, stdout, stderr)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (status, stdout, stderr) = self.run(args);
        assert!(
            status.success(),
            "status={status} stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn write_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.files.join(name);
        fs::write(&path, bytes).expect("fixture file");
        path
    }

    fn index(&self, file: &Path) -> String {
        self.ok(&[
            "--index-file",
            self.data.to_str().expect("data path"),
            file.to_str().expect("file path"),
        ])
    }

    fn search(&self, query: &str) -> String {
        self.ok(&[
            "--search-root",
            self.data.to_str().expect("data path"),
            query,
        ])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    reader
        .take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .expect("drain pipe");
    assert!(
        u64::try_from(bytes.len()).expect("output fits") <= MAX_OUTPUT,
        "test output ceiling exceeded"
    );
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn field<'a>(output: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    output
        .split_once(&needle)
        .expect("field present")
        .1
        .split('"')
        .next()
        .expect("field value")
}

fn matches(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

fn gaps(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"source_gap\""))
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").expect("hex push");
        out
    })
}

/// DIRECT-spine leg support mirrored from
/// `search-query-planner::direct_leg_support` (same closed registry, same
/// arms): `(direct, exact, advanced)`. The planner crate pins its side with
/// `direct_leg_support_covers_all_eleven_without_drift`; this file pins the
/// process side. Both assert the same 9/2/4 counts, so drift fails loudly.
const fn direct_support(recipe: RecipeIdV1) -> (bool, bool, bool) {
    match recipe {
        RecipeIdV1::Locate
        | RecipeIdV1::InspectEntity
        | RecipeIdV1::ExploreEntity
        | RecipeIdV1::CompareImplementations => (true, false, true),
        RecipeIdV1::FindText => (true, true, false),
        RecipeIdV1::CorpusProfile
        | RecipeIdV1::CorpusDelta
        | RecipeIdV1::Provenance
        | RecipeIdV1::ExpandHandle => (true, false, false),
        RecipeIdV1::CompileExactScan | RecipeIdV1::ExecuteExactScan => (false, true, false),
    }
}

#[test]
fn recipe_registry_is_exactly_eleven_versioned() {
    assert_eq!(RecipeIdV1::ALL.len(), 11);
    for recipe in RecipeIdV1::ALL {
        assert_eq!(RecipeIdV1::parse(recipe.as_str()), Ok(recipe));
        assert_eq!(RecipeIdV1::parse_versioned(recipe.as_str()), Ok(recipe));
    }
    // Unversioned aliases and unknown spellings fail closed at the contract.
    for rejected in [
        "",
        "locate",
        "find_text",
        "compare_implementations",
        "locate@2",
        "LOCATE@1",
        "find_text@1 ",
    ] {
        assert!(RecipeIdV1::parse(rejected).is_err(), "{rejected}");
        assert!(RecipeIdV1::parse_versioned(rejected).is_err(), "{rejected}");
    }
    // Body/recipe mismatch fails closed before any planning or retrieval.
    let mismatched = SearchRecipeRequest::new(
        RequestId::from_bytes([0x11; 16]),
        RecipeIdV1::Locate,
        SourceView::RetainedRevision(SourceRevisionId::from_bytes([0x02; 16])),
        RequestedScope::ExplicitMemberships(BoundedList::new(Vec::new()).expect("scope")),
        ProfileId::new("interactive").expect("budget class"),
        RecipeBodyV1::FindText(FindTextRecipe {
            predicate: ExactPredicate {
                kind: ExactPredicateKind::Literal,
                engine_and_version: ProfileId::new("literal-v1").expect("engine"),
                serialized_form: BoundedCanonicalBytes::from_validated(b"needle".to_vec())
                    .expect("predicate bytes"),
                input_domain: ExactInputDomain::DecodedText,
                worst_case_complexity_class: ProfileId::new("linear-scan").expect("class"),
            },
            case_policy: CasePolicy::Exact,
            context_bytes_before: 0,
            context_bytes_after: 0,
        }),
    );
    assert!(mismatched.is_err(), "body/recipe mismatch must fail");
}

#[test]
fn direct_spine_coverage_table_advertises_only_executable_chain() {
    // Every registry member is classified; the match is exhaustive so a
    // twelfth recipe fails compilation here instead of slipping through.
    let mut direct = 0_usize;
    let mut exact_only = 0_usize;
    let mut advanced_omitted = 0_usize;
    for recipe in RecipeIdV1::ALL {
        let (has_direct, has_exact, wants_advanced) = direct_support(recipe);
        if has_direct {
            direct += 1;
        } else {
            assert!(has_exact, "{recipe:?}");
            assert!(!wants_advanced, "{recipe:?}");
            exact_only += 1;
        }
        if wants_advanced {
            advanced_omitted += 1;
            assert!(has_direct, "{recipe:?}");
        }
    }
    assert_eq!(direct, 9, "nine DIRECT-spine recipes");
    assert_eq!(exact_only, 2, "compile/execute_exact_scan stay exact-only");
    assert_eq!(
        advanced_omitted, 4,
        "locate/inspect/explore/compare omit advanced legs explicitly"
    );
    // Spot rows the probe tests below execute.
    assert!(direct_support(RecipeIdV1::FindText).0);
    assert!(direct_support(RecipeIdV1::CorpusProfile).0);
    assert!(direct_support(RecipeIdV1::CorpusDelta).0);
    assert!(direct_support(RecipeIdV1::Provenance).0);
    assert!(direct_support(RecipeIdV1::ExpandHandle).0);
    assert!(!direct_support(RecipeIdV1::CompileExactScan).0);
}

#[test]
fn positive_direct_recipes_prove_source_backed_witnesses() {
    let fixture = Fixture::new();
    assert!(direct_support(RecipeIdV1::FindText).0);
    let alpha_text = format!("crate {} baseline needle", CRATE_DICTIONARY[0]);
    let needle_start = alpha_text.find("needle").expect("fixture needle");
    let needle_end = needle_start + "needle".len();
    let alpha = fixture.write_file("search-comparator-baseline.txt", alpha_text.as_bytes());
    let gamma = fixture.write_file(
        "search-query-planner-baseline.txt",
        format!("crate {} baseline delta", CRATE_DICTIONARY[1]).as_bytes(),
    );
    let indexed = fixture.index(&alpha);
    let revision = field(&indexed, "revision_id").to_owned();
    assert!(indexed.contains("\"changed\":true"), "{indexed}");
    fixture.index(&gamma);

    // find_text@1 positive: source-backed witness over the frozen admitted
    // denominator, complete, no gaps, both sources searched.
    let output = fixture.search("needle");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(
        rows[0].contains(&format!("\"byte_start\":{needle_start},")),
        "{output}"
    );
    assert!(
        rows[0].contains(&format!("\"byte_end\":{needle_end},")),
        "{output}"
    );
    assert!(rows[0].contains("\"source_backed\":true"), "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(output.contains("\"gaps\":0"), "{output}");
    assert!(output.contains("\"searched_sources\":2"), "{output}");
    assert!(output.contains("\"active_sources\":2"), "{output}");

    // corpus_profile@1: list and verify agree on the admitted scope.
    let listed = fixture.ok(&["--list-sources", fixture.data.to_str().expect("data path")]);
    assert!(listed.contains("\"sources\":2"), "{listed}");
    let verified = fixture.ok(&["--verify-root", fixture.data.to_str().expect("data path")]);
    assert!(verified.contains("\"verified_revisions\":2"), "{verified}");
    assert!(
        verified.contains("\"referenced_revisions\":2"),
        "{verified}"
    );

    // provenance@1 / expand_handle@1: the witness reconstructs from exact
    // retained bytes through a provenance-backed excerpt slice.
    let slice = fixture.ok(&[
        "--read-revision",
        fixture.data.to_str().expect("data path"),
        &revision,
        &needle_start.to_string(),
        &needle_end.to_string(),
    ]);
    assert!(slice.contains(&hex_bytes(b"needle")), "{slice}");
    assert!(slice.contains("\"source_backed\":true"), "{slice}");

    // corpus_delta@1: re-indexing unchanged bytes reports no change, while
    // new bytes report a change — the from_view/to_view delta is explicit.
    let reindexed = fixture.index(&alpha);
    assert!(reindexed.contains("\"changed\":false"), "{reindexed}");
    fs::write(&alpha, b"crate search-comparator baseline needle v2").expect("rewrite");
    let changed = fixture.index(&alpha);
    assert!(changed.contains("\"changed\":true"), "{changed}");
}

#[test]
fn denied_admission_and_unsupported_unit_stay_partial_never_complete() {
    let fixture = Fixture::new();
    assert!(direct_support(RecipeIdV1::FindText).0);
    let good = fixture.write_file(
        "search-subject-resolver-baseline.txt",
        format!("crate {} baseline needle", CRATE_DICTIONARY[2]).as_bytes(),
    );
    fixture.index(&good);

    // Refused admission is a typed denial: it never enters the denominator.
    let empty = fixture.write_file("empty.txt", b"");
    let (status, _, stderr) = fixture.run(&[
        "--index-file",
        fixture.data.to_str().expect("data path"),
        empty.to_str().expect("file path"),
    ]);
    assert!(!status.success(), "empty index must not succeed");
    assert!(stderr.contains("SOURCE_ADMISSION_DENIED"), "{stderr}");

    // An admitted but unsupported unit becomes an explicit gap, never silent
    // narrowing: the healthy source still proves its witness as partial data.
    let binary = fixture.write_file("binary.dat", b"a\0b");
    fixture.index(&binary);
    let output = fixture.search("needle");
    assert_eq!(matches(&output).len(), 1, "{output}");
    assert_eq!(gaps(&output).len(), 1, "{output}");
    assert!(
        output.contains("MATERIALIZATION_BINARY_CONTENT"),
        "{output}"
    );
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(output.contains("\"active_sources\":2"), "{output}");

    // Zero matches over a gapped denominator is incomplete data, never a
    // complete negative proof.
    let output = fixture.search("absent-needle-xyz");
    assert!(matches(&output).is_empty(), "{output}");
    assert_eq!(gaps(&output).len(), 1, "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(
        !output.contains("\"complete\":true"),
        "partial outcome must never read as success: {output}"
    );
}

#[test]
fn unsupported_recipe_inputs_fail_closed() {
    let fixture = Fixture::new();
    // Unknown commands fail with a typed usage error, never empty success.
    let (status, _, stderr) = fixture.run(&["--frobnicate"]);
    assert!(!status.success(), "unknown command must fail");
    assert!(stderr.contains("UNKNOWN_ARGUMENT"), "{stderr}");
    // Arity guards hold: a truncated DIRECT command is a usage error.
    let (status, _, stderr) = fixture.run(&["--search-root", fixture.data.to_str().expect("x")]);
    assert!(!status.success(), "truncated command must fail");
    assert!(stderr.contains("USAGE_ERROR"), "{stderr}");
    // Missing inputs fail closed without touching another root.
    let missing = fixture.base.join("no-such-file.txt");
    let (status, _, _) = fixture.run(&[
        "--index-file",
        fixture.data.to_str().expect("data path"),
        missing.to_str().expect("missing path"),
    ]);
    assert!(!status.success(), "missing file must fail");
    let (status, _, _) = fixture.run(&[
        "--search-root",
        fixture.base.join("no-such-root").to_str().expect("x"),
        "needle",
    ]);
    assert!(!status.success(), "missing root must fail");
    // Unknown revisions never synthesize bytes.
    let (status, _, _) = fixture.run(&[
        "--read-revision",
        fixture.data.to_str().expect("data path"),
        "0000000000000000000000000000000000000000000000000000000000000000",
        "0",
        "6",
    ]);
    assert!(!status.success(), "unknown revision must fail");
}

#[test]
fn subject_and_comparison_evidence_is_direct_backed() {
    // locate/inspect/explore/compare have no separate daemon surface: they
    // resolve over these exact DIRECT evidence triples (source, revision,
    // content digest). The ladder, ambiguity and lineage rules over such
    // triples are proven in search-subject-resolver and search-comparator;
    // here the triple itself is proven source-backed and stable.
    let fixture = Fixture::new();
    for recipe in [
        RecipeIdV1::Locate,
        RecipeIdV1::InspectEntity,
        RecipeIdV1::ExploreEntity,
        RecipeIdV1::CompareImplementations,
    ] {
        assert!(direct_support(recipe).0, "{recipe:?}");
        assert!(direct_support(recipe).2, "{recipe:?}");
    }
    let file = fixture.write_file(
        "search-contracts-baseline.txt",
        format!("crate {} baseline needle", CRATE_DICTIONARY[3]).as_bytes(),
    );
    let indexed = fixture.index(&file);
    let source_id = field(&indexed, "source_id").to_owned();
    let revision_id = field(&indexed, "revision_id").to_owned();
    let content_digest = field(&indexed, "content_digest").to_owned();
    assert_eq!(content_digest.len(), 64, "{indexed}");

    // The store verifies every referenced revision it claims evidence over.
    let verified = fixture.ok(&["--verify-root", fixture.data.to_str().expect("data path")]);
    assert!(
        verified.contains("\"referenced_revisions\":1"),
        "{verified}"
    );
    assert!(verified.contains("\"verified_revisions\":1"), "{verified}");

    // The triple is stable across repeated listings and searches.
    let listed = fixture.ok(&["--list-sources", fixture.data.to_str().expect("data path")]);
    assert!(
        listed.contains(&format!("\"source_id\":\"{source_id}\"")),
        "{listed}"
    );
    assert!(
        listed.contains(&format!("\"revision_id\":\"{revision_id}\"")),
        "{listed}"
    );
    let first = fixture.search("needle");
    let second = fixture.search("needle");
    assert_eq!(matches(&first).len(), 1, "{first}");
    assert_eq!(matches(&second).len(), 1, "{second}");
    assert!(
        first.contains(&format!("\"revision_id\":\"{revision_id}\"")),
        "{first}"
    );
    assert!(
        second.contains(&format!("\"content_digest\":\"{content_digest}\"")),
        "{second}"
    );
    assert!(first.contains("\"complete\":true"), "{first}");
}
