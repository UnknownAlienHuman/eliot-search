//! One real Native -> protected Credential Manager -> redb -> restart proof.
use super::*;
use search_os_secrets::RevisionDekBinding;
use search_os_secrets_windows::{self as os, LegacyRevisionDpapiContext, LegacyRevisionDpapiWork};
use std::{cell::{Cell, RefCell}, rc::Rc};

struct CleanupContext { deadline: Instant }
impl CleanupContext { fn new() -> Self { Self { deadline: Instant::now() + Duration::from_secs(30) } } }
impl LegacyRevisionDpapiContext for CleanupContext {
    type Error = &'static str;
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        if Instant::now() < self.deadline { Ok(()) } else { Err("TEST_CLEANUP_DEADLINE") }
    }
    fn charge(&mut self, _: LegacyRevisionDpapiWork) -> Result<(), Self::Error> { self.checkpoint() }
    fn account_native_clear(&mut self, _: usize) -> Result<(), Self::Error> { Ok(()) }
}
impl os::RevisionDekContext for CleanupContext {
    fn deadline(&self) -> Instant { self.deadline }
    fn charge_open(&mut self) -> Result<(), Self::Error> { self.checkpoint() }
}

struct OwnedCandidates {
    targets: Rc<RefCell<Vec<RevisionDekBinding>>>,
    cleaned: Cell<Option<bool>>,
}
impl OwnedCandidates {
    fn new() -> Self { Self { targets: Rc::new(RefCell::new(Vec::new())), cleaned: Cell::new(None) } }
    fn cleanup(&self) -> bool {
        if let Some(outcome) = self.cleaned.get() { return outcome; }
        let mut context = CleanupContext::new();
        let mut outcome = true;
        for binding in self.targets.borrow().iter() {
            outcome &= os::delete_revision_dek_for_test(binding, &mut context).is_ok();
        }
        self.cleaned.set(Some(outcome));
        outcome
    }
}
impl Drop for OwnedCandidates {
    fn drop(&mut self) {
        let outcome = self.cleanup();
        if !outcome {
            let _ = std::io::Write::write_all(&mut std::io::stderr(),
                b"exact candidate credential cleanup unresolved; disk retained\n");
            assert!(std::thread::panicking(), "exact candidate credential cleanup unresolved; disk retained");
        }
    }
}

fn run(s: &Scratch, owned: &OwnedCandidates, operation: &str) -> (Result<(), String>, String) {
    let args = [OsString::from("--prepare-revision-dek-v1"), s.root.as_os_str().to_owned(), operation.into()];
    let deadline = Instant::now() + Duration::from_secs(120);
    let (request, selected) = NativeRootRequest::for_direct_registry(&args,
        DataRootOpenMode::OpenExistingMutating, deadline, Arc::new(AtomicBool::new(false)),
        super::super::super::super::native_request::NativeRootLimits::new(
            [256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256]).unwrap()).unwrap();
    let command = super::super::dek_command::RevisionDekCommand::parse(&selected).unwrap();
    let targets = Rc::clone(&owned.targets);
    let mut work = NativeWork::new();
    work.revision_dek_target_hook = Some(Box::new(move |binding| targets.borrow_mut().push(binding.clone())));
    let owner = DirectRegistryOwner::open_with_work(request, work).unwrap();
    let mut output = Vec::new();
    let result = owner.execute_revision_dek(&command, &mut output);
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn native_dek_candidate_persists_restarts_conflicts_and_cleans_exact_target() {
    let s = Scratch::new(true);
    // Declared after Scratch so candidate cleanup runs before its disk teardown.
    let owned = OwnedCandidates::new();
    let (result, first) = run(&s, &owned, "51515151515151515151515151515151");
    assert!(result.is_ok(), "{first}: {result:?}");
    for field in ["\"prepared\":true", "\"historical_commit\":false", "\"role\":\"candidate_nonserving\"",
        "\"current_selected\":false", "\"key_use\":false", "\"ready\":false"] {
        assert!(first.contains(field), "{first}");
    }
    assert_eq!(owned.targets.borrow().len(), 1, "one possible publication path");
    let binding = owned.targets.borrow()[0].clone();
    let first_read = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    let (result, replay) = run(&s, &owned, "51515151515151515151515151515151");
    assert!(result.is_ok(), "{replay}: {result:?}");
    assert!(replay.contains("\"historical_commit\":true"), "{replay}");
    assert_eq!(owned.targets.borrow().len(), 1, "replay must not enter a publication path");
    let after = os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().unwrap();
    assert!(first_read.protected_bytes() == after.protected_bytes(), "protected key changed on replay");
    let (result, conflict) = run(&s, &owned, "52525252525252525252525252525252");
    assert!(result.is_err());
    assert!(conflict.contains("REVISION_DEK_OPERATION_CONFLICT"), "{conflict}");
    assert_eq!(owned.targets.borrow().len(), 1, "conflict must not enter publication");
    assert!(owned.cleanup(), "exact candidate cleanup");
    assert!(os::read_revision_dek(&binding, &mut CleanupContext::new()).unwrap().is_none(), "candidate still present");
}
