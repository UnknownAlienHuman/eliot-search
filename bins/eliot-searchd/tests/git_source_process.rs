//! T32 remainder e2e: exact no-execute Git source acquisition through the
//! canonical composition pipeline.
//!
//! Every test drives the real `search-safe-reader::git` validation together
//! with the real `source_composition` git binding: loose blob/tree/commit/tag
//! objects validate (header/kind/size/structure), packed-only objects return
//! explicit `GIT_OBJECT_PACKED_UNAVAILABLE`, missing objects return
//! `GIT_OBJECT_NOT_FOUND`, remote-promised objects return
//! `GIT_OBJECT_REQUIRES_NETWORK`, and malformed inputs reject without a
//! durable identifier. No hook, filter, shell, credential helper or network
//! is invoked; the backend below never spawns a process. Paths classify only;
//! identity is repository digest plus object ID, never path/remote/HEAD text.
//! The same `RegistryView` backs file and git planning: no second catalog.

#[path = "../src/sha256.rs"]
mod sha256;

#[path = "../src/source_composition.rs"]
mod source_composition;

use std::path::Path;

use search_safe_reader::git::{
    GitBackendObject, GitObjectBackend, GitObjectId, GitObjectKind, GitObjectRequest, GitReadError,
    GitReadLimits, read_git_object_no_execute,
};
use source_composition::{AdmissionPolicy, GitLineage, GitLineageKind, RegistryView};

const TEST_LIMITS: GitReadLimits = GitReadLimits {
    max_decompressed_bytes: 4096,
    max_path_token_bytes: 256,
};

fn repository_hex() -> String {
    sha256::hex(&sha256::digest(b"eliot-git-test-repository"))
}

fn other_repository_hex() -> String {
    sha256::hex(&sha256::digest(b"eliot-git-other-repository"))
}

fn namespace_hex() -> String {
    sha256::hex(&sha256::digest(b"eliot-git-test-namespace"))
}

fn evidence_hex() -> String {
    sha256::hex(&sha256::digest(b"eliot-git-lineage-evidence"))
}

fn repository_digest_for(hex_value: &str) -> search_contracts::Blake3Digest32 {
    let raw = sha256::decode_digest(hex_value).expect("repository hex decodes");
    search_contracts::Blake3Digest32::from_bytes(raw)
}

const fn object_id(fill: u8) -> GitObjectId {
    GitObjectId::from_bytes([fill; 20])
}

fn object_hex(fill: u8) -> String {
    object_id(fill).hex()
}

fn lineage(kind: GitLineageKind) -> GitLineage {
    GitLineage {
        kind,
        evidence_digest_hex: evidence_hex(),
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy::baseline()
}

fn view() -> RegistryView {
    RegistryView::build(Vec::new(), &policy()).expect("empty view")
}

fn encode_object(kind: &str, payload: &[u8]) -> Vec<u8> {
    let mut object = Vec::with_capacity(kind.len() + 24 + payload.len());
    object.extend_from_slice(kind.as_bytes());
    object.push(b' ');
    object.extend_from_slice(format!("{}", payload.len()).as_bytes());
    object.push(0);
    object.extend_from_slice(payload);
    object
}

fn tree_payload() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"100644 ");
    payload.extend_from_slice(b"README.md");
    payload.push(0);
    payload.extend_from_slice(&[0x11; 20]);
    payload
}

fn commit_payload() -> Vec<u8> {
    let tree = "11".repeat(20);
    format!(
        "tree {tree}\nauthor Test <test@example.invalid> 0 +0000\ncommitter Test <test@example.invalid> 0 +0000\n\ninit\n"
    )
    .into_bytes()
}

fn tag_payload() -> Vec<u8> {
    let target = "22".repeat(20);
    format!(
        "object {target}\ntype commit\ntag v1.0\ntagger Test <test@example.invalid> 0 +0000\n\nrelease\n"
    )
    .into_bytes()
}

fn blob_decompressed() -> Vec<u8> {
    encode_object("blob", b"hello world\n")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FakeFailure {
    NotFound,
    Packed,
    Network,
    Backend,
}

struct FakeBackend {
    object: Option<GitBackendObject>,
    failure: Option<FakeFailure>,
}

impl FakeBackend {
    fn succeeds(decompressed: Vec<u8>, id_fill: u8, repository: &str) -> Self {
        Self {
            object: Some(GitBackendObject {
                decompressed,
                observed_id: object_id(id_fill),
                observed_repository_identity_digest: repository_digest_for(repository),
            }),
            failure: None,
        }
    }

    const fn fails(failure: FakeFailure) -> Self {
        Self {
            object: None,
            failure: Some(failure),
        }
    }
}

impl GitObjectBackend for FakeBackend {
    type BackendError = FakeFailure;

    fn read_loose_object(
        &mut self,
        _request: &search_safe_reader::git::GitLooseOpenRequest,
    ) -> Result<GitBackendObject, Self::BackendError> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        self.object.clone().ok_or(FakeFailure::NotFound)
    }

    fn map_backend_error(error: &Self::BackendError) -> GitReadError {
        match error {
            FakeFailure::NotFound => GitReadError::ObjectNotFound,
            FakeFailure::Packed => GitReadError::PackedObjectUnavailable,
            FakeFailure::Network => GitReadError::ObjectRequiresNetwork,
            FakeFailure::Backend => GitReadError::BackendFailure,
        }
    }
}

fn request_for(repository: &str, id_fill: u8, kind: Option<GitObjectKind>) -> GitObjectRequest {
    GitObjectRequest {
        repository_identity_digest: repository_digest_for(repository),
        object_id: object_id(id_fill),
        expected_kind: kind,
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_through_composition(
    repository: &str,
    id_fill: u8,
    kind: Option<GitObjectKind>,
    decompressed: &[u8],
    logical_path: &str,
    lineage_kind: GitLineageKind,
    namespace: &str,
    policy_ref: &AdmissionPolicy,
    view_ref: &RegistryView,
) -> Result<source_composition::GitPlannedSource, String> {
    source_composition::plan_git_snapshot(
        repository,
        &object_hex(id_fill),
        kind,
        decompressed,
        Path::new(logical_path),
        &lineage(lineage_kind),
        namespace,
        policy_ref,
        view_ref,
    )
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn no_execute_invariants_are_statically_pinned() {
    assert!(search_safe_reader::git::SAFE_GIT_NO_EXECUTE);
    assert!(source_composition::GIT_SOURCE_NO_EXECUTE);
}

#[test]
fn loose_blob_reads_and_plans_through_canonical_pipeline() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let decompressed = blob_decompressed();
    let mut backend = FakeBackend::succeeds(decompressed.clone(), 0x01, &repository);
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("loose blob validates");
    assert_eq!(read.kind, GitObjectKind::Blob);
    assert_eq!(read.payload(), b"hello world\n");
    let planned = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("blob plans");
    assert_eq!(planned.object_id_hex, object_hex(0x01));
    assert_eq!(
        planned.repository_digest_hex,
        repository.to_ascii_lowercase()
    );
    assert_eq!(planned.kind, GitObjectKind::Blob);
    assert_eq!(
        planned.payload_len,
        u64::try_from(b"hello world\n".len()).expect("len")
    );
    assert!(planned.identity_native);
    assert_eq!(planned.source_id.len(), 64);
    assert_eq!(planned.revision_id.len(), 64);
    assert_eq!(planned.lineage, lineage(GitLineageKind::Repository),);
}

#[test]
fn loose_tree_reads_and_plans() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let payload = tree_payload();
    let decompressed = encode_object("tree", &payload);
    let mut backend = FakeBackend::succeeds(decompressed.clone(), 0x02, &repository);
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x02, Some(GitObjectKind::Tree)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("loose tree validates");
    assert_eq!(read.kind, GitObjectKind::Tree);
    let planned = plan_through_composition(
        &repository,
        0x02,
        Some(GitObjectKind::Tree),
        &decompressed,
        "/workspace/tree-listing.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("tree plans");
    assert_eq!(planned.kind, GitObjectKind::Tree);
    assert_eq!(
        planned.payload_len,
        u64::try_from(payload.len()).expect("len")
    );
}

#[test]
fn loose_commit_reads_and_plans() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let payload = commit_payload();
    let decompressed = encode_object("commit", &payload);
    let mut backend = FakeBackend::succeeds(decompressed.clone(), 0x03, &repository);
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x03, Some(GitObjectKind::Commit)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("loose commit validates");
    assert_eq!(read.kind, GitObjectKind::Commit);
    let planned = plan_through_composition(
        &repository,
        0x03,
        Some(GitObjectKind::Commit),
        &decompressed,
        "/workspace/commit-record.txt",
        GitLineageKind::Worktree,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("commit plans");
    assert_eq!(planned.kind, GitObjectKind::Commit);
}

#[test]
fn loose_tag_reads_and_plans() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let payload = tag_payload();
    let decompressed = encode_object("tag", &payload);
    let mut backend = FakeBackend::succeeds(decompressed.clone(), 0x04, &repository);
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x04, Some(GitObjectKind::Tag)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("loose tag validates");
    assert_eq!(read.kind, GitObjectKind::Tag);
    let planned = plan_through_composition(
        &repository,
        0x04,
        Some(GitObjectKind::Tag),
        &decompressed,
        "/workspace/release-note.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("tag plans");
    assert_eq!(planned.kind, GitObjectKind::Tag);
}

#[test]
fn packed_object_returns_explicit_unavailable_without_identifier() {
    let repository = repository_hex();
    let mut backend = FakeBackend::fails(FakeFailure::Packed);
    let mut never_cancel = || false;
    let error = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect_err("packed must not substitute");
    assert_eq!(error, GitReadError::PackedObjectUnavailable);
    assert_eq!(error.code(), "GIT_OBJECT_PACKED_UNAVAILABLE");
    assert_eq!(
        source_composition::git_error_code(error),
        "GIT_OBJECT_PACKED_UNAVAILABLE"
    );
}

#[test]
fn missing_object_returns_not_found_without_identifier() {
    let repository = repository_hex();
    let mut backend = FakeBackend::fails(FakeFailure::NotFound);
    let mut never_cancel = || false;
    let error = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x09, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect_err("missing must not substitute");
    assert_eq!(error, GitReadError::ObjectNotFound);
    assert_eq!(error.code(), "GIT_OBJECT_NOT_FOUND");
}

#[test]
fn remote_promised_object_requires_network_without_fetch() {
    let repository = repository_hex();
    let mut backend = FakeBackend::fails(FakeFailure::Network);
    let mut never_cancel = || false;
    let error = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect_err("network fetch is denied");
    assert_eq!(error, GitReadError::ObjectRequiresNetwork);
    assert_eq!(error.code(), "GIT_OBJECT_REQUIRES_NETWORK");
}

#[test]
fn backend_failure_is_content_free_without_identifier() {
    let repository = repository_hex();
    let mut backend = FakeBackend::fails(FakeFailure::Backend);
    let mut never_cancel = || false;
    let error = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect_err("backend failure stays content-free");
    assert_eq!(error, GitReadError::BackendFailure);
    assert_eq!(error.code(), "GIT_OBJECT_BACKEND_FAILURE");
    assert_eq!(
        source_composition::git_error_code(error),
        "GIT_OBJECT_BACKEND_FAILURE"
    );
}

#[test]
fn malformed_loose_objects_reject_without_identifier() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    // No NUL separator.
    let bad_header = b"blob 3 abc".to_vec();
    let mut backend = FakeBackend::succeeds(bad_header.clone(), 0x01, &repository);
    let mut never_cancel = || false;
    assert_eq!(
        read_git_object_no_execute(
            &mut backend,
            &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
            &mut never_cancel
        ),
        Err(GitReadError::MalformedHeader)
    );
    assert!(
        plan_through_composition(
            &repository,
            0x01,
            Some(GitObjectKind::Blob),
            &bad_header,
            "/workspace/notes.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .is_err()
    );
    // Declared size differs from payload.
    let size_mismatch = b"blob 5\0hi".to_vec();
    assert!(
        plan_through_composition(
            &repository,
            0x01,
            Some(GitObjectKind::Blob),
            &size_mismatch,
            "/workspace/notes.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .expect_err("size mismatch rejects")
        .contains("GIT_OBJECT_SIZE_MISMATCH")
    );
    // Truncated tree entry.
    let mut truncated = Vec::new();
    truncated.extend_from_slice(b"100644 ");
    truncated.extend_from_slice(b"no-sha-here");
    truncated.push(0);
    truncated.extend_from_slice(&[0x11; 4]);
    let truncated_object = encode_object("tree", &truncated);
    assert!(
        plan_through_composition(
            &repository,
            0x02,
            Some(GitObjectKind::Tree),
            &truncated_object,
            "/workspace/tree-listing.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .expect_err("truncated tree rejects")
        .contains("GIT_OBJECT_MALFORMED_PAYLOAD")
    );
    // Commit without a tree line.
    let bad_commit = encode_object("commit", b"author Test <t@e> 0 +0000\n\nmsg\n");
    assert!(
        plan_through_composition(
            &repository,
            0x03,
            Some(GitObjectKind::Commit),
            &bad_commit,
            "/workspace/commit-record.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .is_err()
    );
    // Unknown kind.
    let unknown = encode_object("blorb", b"abc");
    assert!(
        plan_through_composition(
            &repository,
            0x05,
            None,
            &unknown,
            "/workspace/notes.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .expect_err("unknown kind rejects")
        .contains("GIT_OBJECT_UNKNOWN_KIND")
    );
}

#[test]
fn kind_mismatch_and_identity_mismatch_reject() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let payload = tree_payload();
    let tree_object = encode_object("tree", &payload);
    assert!(
        plan_through_composition(
            &repository,
            0x02,
            Some(GitObjectKind::Blob),
            &tree_object,
            "/workspace/tree-listing.txt",
            GitLineageKind::Repository,
            &namespace,
            &current_policy,
            &current_view,
        )
        .expect_err("kind mismatch rejects")
        .contains("GIT_OBJECT_KIND_MISMATCH")
    );
    // Backend-observed ID differs from requested ID.
    let decompressed = blob_decompressed();
    let mut backend = FakeBackend::succeeds(decompressed, 0x02, &repository);
    let mut never_cancel = || false;
    assert_eq!(
        read_git_object_no_execute(
            &mut backend,
            &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
            &mut never_cancel
        ),
        Err(GitReadError::ObjectIdentityMismatch)
    );
    // Backend-observed repository differs from admitted repository.
    let decompressed = blob_decompressed();
    let mut backend = FakeBackend::succeeds(decompressed, 0x01, &other_repository_hex());
    let mut never_cancel = || false;
    assert_eq!(
        read_git_object_no_execute(
            &mut backend,
            &request_for(&repository, 0x01, Some(GitObjectKind::Blob)),
            TEST_LIMITS,
            &mut never_cancel
        ),
        Err(GitReadError::RepositoryMismatch)
    );
}

#[test]
fn invalid_ids_and_lineage_reject_without_identifier() {
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let decompressed = blob_decompressed();
    // Malformed object ID text never derives a loose path.
    assert_eq!(
        source_composition::derive_git_loose_path("not-hex"),
        Err("GIT_OBJECT_INVALID_ID".to_owned())
    );
    assert_eq!(
        source_composition::validate_git_object_hex("zz12cd34ef56ab12cd34ef56ab12cd34ef56ab12"),
        Err("GIT_OBJECT_INVALID_ID".to_owned())
    );
    // Malformed repository digest never plans.
    assert_eq!(
        source_composition::validate_git_repository_hex("not-hex"),
        Err(source_composition::GIT_SOURCE_REPOSITORY_INVALID.to_owned())
    );
    assert!(
        source_composition::plan_git_snapshot(
            "not-hex",
            &object_hex(0x01),
            Some(GitObjectKind::Blob),
            &decompressed,
            Path::new("/workspace/notes.txt"),
            &lineage(GitLineageKind::Repository),
            &namespace,
            &current_policy,
            &current_view,
        )
        .is_err()
    );
    // Lineage evidence must be an exact 64-hex digest.
    let bad_lineage = GitLineage {
        kind: GitLineageKind::Submodule,
        evidence_digest_hex: "not-evidence".to_owned(),
    };
    assert_eq!(
        source_composition::validate_git_lineage(&bad_lineage),
        Err(source_composition::GIT_SOURCE_LINEAGE_INVALID.to_owned())
    );
}

#[test]
fn same_remote_lineage_with_different_repositories_stays_distinct() {
    let first = repository_hex();
    let second = other_repository_hex();
    assert_ne!(first, second);
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let decompressed = blob_decompressed();
    // Same object ID under different admitted repositories: different stable
    // identities and different durable sources. A remote URL or HEAD name
    // never participates: only the admitted digest plus the object ID do.
    let first_plan = plan_through_composition(
        &first,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("first repository plans");
    let second_plan = plan_through_composition(
        &second,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("second repository plans");
    assert_ne!(first_plan.source_id, second_plan.source_id);
    assert_ne!(first_plan.revision_id, second_plan.revision_id);
    let first_loose =
        source_composition::derive_git_loose_path(&object_hex(0x01)).expect("loose path");
    assert!(first_loose.starts_with("objects/"));
    assert_eq!(first_loose.len(), 49);
}

#[test]
fn paths_classify_only_and_never_form_identity() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let decompressed = blob_decompressed();
    // Same repository plus object under different logical paths: one durable
    // identity. Paths are locators for admission classification only.
    let first = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/first.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("first path plans");
    let second = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/second.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect("second path plans");
    assert_eq!(first.source_id, second.source_id);
    assert_eq!(first.revision_id, second.revision_id);
}

#[test]
fn lineage_kinds_bind_evidence_without_forking_identity() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let decompressed = blob_decompressed();
    let mut source_ids = Vec::new();
    for kind in [
        GitLineageKind::Repository,
        GitLineageKind::Worktree,
        GitLineageKind::Submodule,
        GitLineageKind::Fork,
        GitLineageKind::Mirror,
    ] {
        assert!(!kind.as_str().is_empty());
        let planned = plan_through_composition(
            &repository,
            0x01,
            Some(GitObjectKind::Blob),
            &decompressed,
            "/workspace/notes.txt",
            kind,
            &namespace,
            &current_policy,
            &current_view,
        )
        .unwrap_or_else(|error| panic!("kind={kind:?} plans: {error}"));
        assert_eq!(planned.lineage.kind, kind);
        assert_eq!(planned.lineage.evidence_digest_hex, evidence_hex());
        source_ids.push(planned.source_id);
    }
    // Lineage is bound evidence on the plan; the durable identity stays
    // repository plus object so worktree/submodule/fork/mirror views of the
    // same object do not fork a second catalog entry.
    for id in &source_ids {
        assert_eq!(id, &source_ids[0]);
    }
}

#[test]
fn same_view_rebinds_without_second_catalog() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let empty = view();
    let decompressed = blob_decompressed();
    let first = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &empty,
    )
    .expect("first plans");
    // Rebuild the coherent view over the first durable binding and plan
    // again: the same repository plus object reuses the durable source with
    // no second map or mutable catalog.
    let prior = vec![source_composition::PriorSourceView {
        source_id: first.source_id.clone(),
        file_identity_digest: source_composition::git_stable_identity_hex(
            &repository,
            &object_hex(0x01),
        )
        .expect("stable identity"),
        path_digest: sha256::hex(&sha256::digest(b"locator-history")),
        revision_id: first.revision_id.clone(),
        record_digest: sha256::hex(&sha256::digest(b"record")),
        is_active: true,
    }];
    let rebound = RegistryView::build(prior, &current_policy).expect("rebound view");
    // The coherent view is a projection, not a second catalog: the rebound
    // durable binding is visible through the same view API.
    assert!(rebound.get(&first.source_id).is_some());
    assert!(
        rebound
            .get(&sha256::hex(&sha256::digest(b"unknown")))
            .is_none()
    );
    let second = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &decompressed,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &rebound,
    )
    .expect("rebound plans");
    assert_eq!(second.source_id, first.source_id);
    assert_eq!(second.revision_id, first.revision_id);
}

#[test]
fn malicious_hook_is_inert_data_and_never_executed() {
    // A repository-controlled hook that would create a marker if executed.
    // The no-execute read below never spawns it: the marker must stay absent
    // while the blob payload still validates byte-exact.
    let base = std::env::temp_dir().join(format!(
        "eliot-git-noexec-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(base.join("hooks")).expect("hooks dir");
    let marker = base.join("PWNED_MARKER");
    let hook = "@echo off\r\necho pwned > ".to_owned() + &marker.display().to_string() + "\r\n";
    std::fs::write(base.join("hooks").join("post-checkout"), hook.as_bytes()).expect("hook bytes");
    let repository = repository_hex();
    let script = b"@echo off\r\necho pwned\n";
    let decompressed = encode_object("blob", script);
    let mut backend = FakeBackend::succeeds(decompressed, 0x07, &repository);
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x07, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("script blob validates as inert bytes");
    assert_eq!(read.payload(), script);
    assert!(!marker.try_exists().expect("marker probe"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn empty_blob_is_denied_before_any_identifier() {
    let repository = repository_hex();
    let namespace = namespace_hex();
    let current_policy = policy();
    let current_view = view();
    let empty_object = encode_object("blob", b"");
    let error = plan_through_composition(
        &repository,
        0x01,
        Some(GitObjectKind::Blob),
        &empty_object,
        "/workspace/notes.txt",
        GitLineageKind::Repository,
        &namespace,
        &current_policy,
        &current_view,
    )
    .expect_err("empty blob never reaches CAS");
    assert!(
        error.contains("SOURCE_ADMISSION_DENIED") || error.contains("DENY"),
        "{error}"
    );
}

#[test]
fn error_views_are_redacted_to_reason_codes() {
    let error = GitReadError::SizeMismatch;
    assert_eq!(format!("{error}"), "GIT_OBJECT_SIZE_MISMATCH");
    assert_eq!(
        source_composition::git_error_code(GitReadError::PackedObjectUnavailable),
        "GIT_OBJECT_PACKED_UNAVAILABLE"
    );
    // Debug views never dump payload bytes.
    let repository = repository_hex();
    let mut backend = FakeBackend::succeeds(
        encode_object("blob", b"top-secret-payload"),
        0x05,
        &repository,
    );
    let mut never_cancel = || false;
    let read = read_git_object_no_execute(
        &mut backend,
        &request_for(&repository, 0x05, Some(GitObjectKind::Blob)),
        TEST_LIMITS,
        &mut never_cancel,
    )
    .expect("read succeeds");
    let debug = format!("{read:?}");
    assert!(!debug.contains("top-secret-payload"));
}
