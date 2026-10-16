//! Pass-local directory custody. It never caches file permission or body bytes.
use super::{Component, DIRECTORY_NAMES, File, MAX_PARENTS, MigrationSource, NAME_BYTES,
    Observed, Path, PathBuf, RefCell, Status, Work, changed, limit, metadata, native_length, open_nofollow};
use eliot_searchd::native_file::{self, ChildMembershipScratch, Observation, ObservationError, QualifiedDirectory};
use std::sync::Arc;

struct Pin {
    path: PathBuf,
    file: QualifiedDirectory,
    native: Observation,
    parent: Option<usize>,
}

pub(in super::super) struct ParentReadScope {
    pins: Vec<Pin>,
    scratch: Vec<ChildMembershipScratch>,
}

impl ParentReadScope {
    pub(in super::super) const fn new() -> Self {
        Self { pins: Vec::new(), scratch: Vec::new() }
    }

    pub(super) fn parent(&mut self, source: &MigrationSource<'_, '_, '_>, path: &Path)
        -> Result<Result<usize, Status>, String> {
        self.resolve_directory(source, path.parent().ok_or_else(changed)?)
    }

    pub(super) fn directory(&mut self, source: &MigrationSource<'_, '_, '_>, path: &Path)
        -> Result<Result<(usize, Arc<File>), Status>, String> {
        source.charge(Work::Compare(2 * std::mem::size_of::<usize>()))?;
        Ok(self.resolve_directory(source, path)?.map(|index| {
            (index, self.pins[index].file.retain_file())
        }))
    }

    /// Retain the already qualified chain without cloning an OS handle. The
    /// caller still takes its own complete before/after directory snapshots.
    pub(super) fn retain_chain(&mut self, source: &MigrationSource<'_, '_, '_>, path: &Path)
        -> Result<Result<Vec<Arc<File>>, Status>, String> {
        let mut index = match self.resolve_directory(source, path)? {
            Ok(index) => index,
            Err(status) => return Ok(Err(status)),
        };
        source.charge(Work::Allocate(MAX_PARENTS * std::mem::size_of::<Arc<File>>()))?;
        source.charge(Work::Compare(MAX_PARENTS * 2 * std::mem::size_of::<usize>()))?;
        let mut chain = Vec::new();
        chain.try_reserve_exact(MAX_PARENTS).map_err(|_| limit())?;
        loop {
            if chain.len() == MAX_PARENTS { return Err(limit()); }
            let pin = self.pins.get(index).ok_or_else(changed)?;
            chain.push(pin.file.retain_file());
            match pin.parent {
                Some(parent) => index = parent,
                None => break,
            }
        }
        chain.reverse();
        Ok(Ok(chain))
    }

    fn resolve_directory(&mut self, source: &MigrationSource<'_, '_, '_>, path: &Path)
        -> Result<Result<usize, Status>, String> {
        let root = source.inventory_root();
        let relative = path.strip_prefix(root).map_err(|_| changed())?;
        source.charge(Work::AllocateLocator(native_length(root.as_os_str()) + MAX_PARENTS * NAME_BYTES))?;
        let mut current = root.to_path_buf();
        let mut parent = None;
        let mut count = 0;
        for component in std::iter::once(None).chain(relative.components().map(Some)) {
            if let Some(component) = component {
                let Component::Normal(name) = component else { return Err(changed()); };
                current.push(name);
            }
            count += 1;
            if count > MAX_PARENTS { return Err(limit()); }
            source.charge(Work::Compare(self.pins.len().saturating_mul(native_length(current.as_os_str()))))?;
            let index = match self.pins.iter().position(|pin| pin.path == current) {
                Some(index) if self.pins[index].parent == parent => index,
                Some(_) => return Err(changed()),
                None => match self.pin(source, &current, parent)? {
                    Ok(index) => index,
                    Err(status) => return Ok(Err(status)),
                },
            };
            parent = Some(index);
        }
        let index = parent.ok_or_else(changed)?;
        self.validate(source, index)?;
        Ok(Ok(index))
    }

    fn pin(&mut self, source: &MigrationSource<'_, '_, '_>, path: &Path, parent: Option<usize>)
        -> Result<Result<usize, Status>, String> {
        if self.pins.len() == DIRECTORY_NAMES { return Err(limit()); }
        if self.pins.len() == self.pins.capacity() {
            let capacity = self.pins.capacity().saturating_mul(2).clamp(8, DIRECTORY_NAMES);
            source.charge(Work::Allocate(capacity * std::mem::size_of::<Pin>()))?;
            source.charge(Work::Compare(self.pins.len() * std::mem::size_of::<Pin>()))?;
            self.pins.try_reserve_exact(capacity - self.pins.len()).map_err(|_| limit())?;
        }
        match metadata(source, path)? {
            Observed::Missing => return Ok(Err(Status::AbsentProven)),
            Observed::Refused(status) => return Ok(Err(status)),
            Observed::Object(meta) if meta.is_dir() => {},
            Observed::Object(_) => return Ok(Err(Status::Corrupt)),
        }
        source.charge(Work::Open)?;
        let opened = open_nofollow(path, true);
        source.checkpoint()?;
        let Ok(file) = opened else { return Ok(Err(Status::Unreadable)); };
        source.charge(Work::Allocate(std::mem::size_of::<File>() + 2 * std::mem::size_of::<usize>()))?;
        let file = Arc::new(file);
        let observed = closed_observer(source, |check| QualifiedDirectory::qualify_with_check(Arc::clone(&file), check))?;
        let (file, mut native) = match observed {
            Ok(Some(qualified)) => qualified,
            Ok(None) => return Ok(Err(Status::Corrupt)),
            Err(_) => return Ok(Err(Status::Gap)),
        };
        if let Some(parent) = parent {
            self.admit_scratch(source)?;
            native = membership(source, self.pins[parent].file.file(), file.file(), &mut self.scratch[0], true)?;
        } else if let Some(expected) = source.root_observation()?
            && !same_identity(native, expected) { return Err(changed()); }
        source.charge(Work::AllocateLocator(native_length(path.as_os_str())))?;
        let index = self.pins.len();
        self.pins.push(Pin { path: path.to_owned(), file, native, parent });
        Ok(Ok(index))
    }

    fn admit_scratch(&mut self, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> {
        if self.scratch.is_empty() {
            let bytes = native_file::CHILD_MEMBERSHIP_BUFFER_BYTES;
            source.charge(Work::Allocate(bytes))?;
            source.charge(Work::Compare(bytes.saturating_mul(2)))?;
            self.scratch.try_reserve_exact(1).map_err(|_| limit())?;
            self.scratch.push(ChildMembershipScratch::new());
        }
        Ok(())
    }

    pub(super) fn validate(&self, source: &MigrationSource<'_, '_, '_>, mut index: usize)
        -> Result<(), String> {
        closed_observer(source, |check| {
            for _ in 0..MAX_PARENTS {
                let pin = self.pins.get(index).ok_or(ObservationError::ObservationFailed)?;
                let fresh = pin.file.identity_with_check(check)?;
                if fresh.attributes & 0x10 == 0 || fresh.volume_serial != pin.native.volume_serial
                    || fresh.file_index != pin.native.file_index || fresh.creation_time != pin.native.creation_time
                { return Err(ObservationError::ObservationFailed); }
                match pin.parent { Some(parent) => index = parent, None => return Ok(()) }
            }
            Err(ObservationError::ObservationFailed)
        })?.map_err(|_| changed())
    }

    pub(super) fn leaf(&mut self, source: &MigrationSource<'_, '_, '_>, index: usize, file: &File)
        -> Result<(), String> {
        self.validate(source, index)?;
        self.admit_scratch(source)?;
        membership(source, self.pins.get(index).ok_or_else(changed)?.file.file(),
            file, &mut self.scratch[0], false).map(|_| ())
    }
}

// Only synchronous borrowed-FD observers belong here. Their ABI hooks reuse the
// original request/meter; all registration/pending checks bracket the closed group.
fn closed_observer<T>(
    source: &MigrationSource<'_, '_, '_>,
    observe: impl FnOnce(&dyn Fn(bool) -> Result<(), ObservationError>) -> Result<T, ObservationError>,
) -> Result<Result<T, ObservationError>, String> {
    source.checkpoint()?;
    let first = RefCell::new(None);
    let check = |before| {
        let result = if before { source.held_observer_dispatch() } else { source.poll() };
        result.map_err(|error| {
            first.borrow_mut().get_or_insert(error);
            ObservationError::ObservationFailed
        })
    };
    let result = observe(&check);
    let checkpoint = source.checkpoint();
    if let Some(error) = first.into_inner() { return Err(error); }
    checkpoint?;
    Ok(result)
}

const fn same_identity(left: Observation, right: Observation) -> bool {
    left.volume_serial == right.volume_serial && left.file_index == right.file_index
        && left.creation_time == right.creation_time
}

fn membership(source: &MigrationSource<'_, '_, '_>, parent: &File, child: &File,
    scratch: &mut ChildMembershipScratch, directory: bool,
) -> Result<Observation, String> {
    source.checkpoint()?;
    let first = RefCell::new(None);
    let reserve = |result: Result<(), String>| result.map_err(|error| {
        if first.borrow().is_none() { *first.borrow_mut() = Some(error); }
        ObservationError::ObservationFailed
    });
    let check = |before| reserve(if before { source.held_observer_dispatch() } else { source.poll() });
    let process = |bytes| reserve(source.charge(Work::Compare(bytes)));
    let verify = if directory { native_file::verify_direct_directory_child_with_check }
        else { native_file::verify_direct_file_child_with_check };
    let result = verify(parent, child, scratch, native_file::CHILD_MEMBERSHIP_MAX_ENTRIES, &check, &process);
    let checkpoint = source.checkpoint();
    if let Some(error) = first.into_inner() { return Err(error); }
    checkpoint?;
    result.map_err(|error| if error == ObservationError::DirectoryEntryLimit { limit() } else { changed() })
}

#[cfg(all(test, windows))]
mod held_chain_tests {
    use super::*;
    use std::cell::Cell;
    use std::fs::{self, FileTimes, OpenOptions};
    use std::os::windows::fs::{FileTimesExt, OpenOptionsExt};
    use std::rc::Rc;

    #[test]
    fn native_held_chain_two_parent_dispatches_and_creation_time_refusal() {
        let armed = Rc::new(Cell::new(false));
        let trace = Rc::new(RefCell::new(Vec::new()));
        let hook_armed = Rc::clone(&armed);
        let hook_trace = Rc::clone(&trace);
        let outcome = super::super::native_v2_held_metadata_tests::with_main_held_metadata_fixture(
            Box::new(move |phase| {
                if hook_armed.get() { hook_trace.borrow_mut().push(phase); }
            }),
            |source| {
                let mut scope = ParentReadScope::new();
                let control = source.inventory_root().join("control");
                let index = scope.resolve_directory(source, &control)?.map_err(|_| changed())?;
                assert_eq!(scope.pins[index].parent, Some(0));
                armed.set(true);
                scope.validate(source, index)?;
                armed.set(false);
                assert_eq!(&*trace.borrow(), &[
                    "migration-checkpoint", "migration-held-observer-dispatch",
                    "migration-held-observer-dispatch", "migration-checkpoint",
                ], "one original-context bracket and one actual ABI reservation per parent");

                // The external scratch adversary changes identity metadata, not
                // source bodies. Restore it before native fixture teardown.
                let writer = OpenOptions::new().access_mode(0x100).share_mode(7)
                    .custom_flags(0x0220_0000).open(&control).unwrap();
                let original = writer.metadata().unwrap().created().unwrap();
                writer.set_times(FileTimes::new().set_created(original + std::time::Duration::from_secs(2))).unwrap();
                let refusal = scope.validate(source, index);
                writer.set_times(FileTimes::new().set_created(original)).unwrap();
                assert_eq!(refusal, Err(changed()), "same file ID cannot hide changed creation time");
                scope.validate(source, index)?;
                Ok(())
            },
        );
        assert_eq!(outcome, Ok(()));
    }

    #[test]
    fn native_held_chain_pending_between_parents_returns_no_proof_and_stays_terminal() {
        use search_runtime_owner::OwnerError;

        let armed = Rc::new(Cell::new(false));
        let dispatches = Rc::new(Cell::new(0));
        let pending = Rc::new(RefCell::new(None::<PathBuf>));
        let hook_armed = Rc::clone(&armed);
        let hook_dispatches = Rc::clone(&dispatches);
        let hook_pending = Rc::clone(&pending);
        let outcome = super::super::native_v2_held_metadata_tests::with_main_held_metadata_fixture(
            Box::new(move |phase| {
                if !hook_armed.get() || phase != "migration-held-observer-dispatch" { return; }
                let count = hook_dispatches.get() + 1;
                hook_dispatches.set(count);
                if count == 2 {
                    // First parent's real GFIBH has completed. No checkpoint
                    // is inserted between parents, so the second ABI still runs.
                    let path = hook_pending.borrow();
                    let mut file = OpenOptions::new().write(true).create_new(true)
                        .open(path.as_ref().unwrap()).unwrap();
                    std::io::Write::write_all(&mut file, b"pending between held parents").unwrap();
                    file.sync_all().unwrap();
                }
            }),
            |source| {
                let mut scope = ParentReadScope::new();
                let index = scope.resolve_directory(source, &source.inventory_root().join("control"))?
                    .map_err(|_| changed())?;
                let path = source.inventory_root().join("control/source-roots.tmp");
                assert!(!path.exists());
                *pending.borrow_mut() = Some(path.clone());
                armed.set(true);
                let refusal = scope.validate(source, index);
                armed.set(false);
                fs::remove_file(&path).expect("remove only this adversary's pending sidecar");
                assert_eq!(refusal, Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned()));
                assert_eq!(dispatches.get(), 2);
                assert_eq!(scope.validate(source, index), refusal);
                assert_eq!(source.poll(), refusal);
                assert_eq!(dispatches.get(), 2, "no resumed ABI after removing pending");
                Ok(())
            },
        );
        assert_eq!(outcome, Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned()));
    }

    #[test]
    fn native_held_chain_original_cancel_between_parents_is_sticky() {
        use crate::direct_store::migration_context::{MigrationContext, NativeMigrationInput};
        use crate::owner_composition::{NativeRootAdmission, NativeRootLimits, NativeRootRequest,
            admit_existing, initialize_new};
        use crate::revision_protection::{RevisionProtector, TestCredentialGuard};
        use search_runtime_owner::{DataRootOpenMode, OwnerError};
        use std::ffi::OsString;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

        // This original request's cancellation Arc belongs to the test. Invoke
        // the actual initializer/credential owners; no initializer or DPAPI logic
        // is implemented here, and no request is issued after cancellation.
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = temp.join(format!("search-held-chain-cancel-{}-{stamp}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let root = base.join("data");
        let output = base.join("output");
        let sources = base.join("sources");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&output).unwrap();
        fs::create_dir(&sources).unwrap();
        let limits = || NativeRootLimits::new([256, 256 * 1024, 512 * 1024,
            1024 * 1024 * 1024, 16384, 256]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let credentials = TestCredentialGuard::for_native_data_root(&root);
        let initialized = initialize_new(NativeRootRequest::new(DataRootOpenMode::InitializeNew,
            &[OsString::from("--initialize-data-root"), root.as_os_str().to_owned(),
                OsString::from("02690269026902690269026902690269")],
            deadline, Arc::new(AtomicBool::new(false)), limits()).unwrap())
            .unwrap();
        let namespace = fs::read(root.join("control/namespace.id")).unwrap();
        let namespace_id = crate::sha256::decode_digest(std::str::from_utf8(&namespace).unwrap().trim()).unwrap();
        assert_eq!(credentials.capture_native_namespace().unwrap(), Some(namespace_id));
        initialized.emit(&mut Vec::new()).unwrap();
        let registration = search_source_registry::encode_legacy_source_root_catalog(
            [sources.to_str().unwrap()]).unwrap();
        let mut catalog = OpenOptions::new().write(true).create_new(true)
            .open(root.join("control/source-roots.v1")).unwrap();
        std::io::Write::write_all(&mut catalog, &registration).unwrap();
        catalog.sync_all().unwrap();
        drop(catalog);
        let evidence = || [".eliot-search-installation.v1", ".eliot-search-initialization-journal.v2",
            ".eliot-search-owner-state-a.v1", ".eliot-search-owner-state-b.v1",
            ".eliot-search-owner.lock", ".eliot-search-sealed-owner.lock",
            "control/namespace.id", "control/source-events.log", "control/source-roots.v1"]
            .map(|path| fs::read(root.join(path)).unwrap());
        let before = evidence();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (request, _) = NativeRootRequest::for_source_control_migration(
            &[OsString::from("--stage-source-control-migration"),
                root.as_os_str().to_owned(), OsString::from("10000000-0000-0000-0000-000000000269"),
                output.as_os_str().to_owned()], deadline, Arc::clone(&cancelled), limits()).unwrap();
        let NativeRootAdmission::Inspection(owner) = admit_existing(request).unwrap() else {
            panic!("actual initialized native read custody");
        };
        let armed = Rc::new(Cell::new(false));
        let dispatches = Rc::new(Cell::new(0));
        let hook_armed = Rc::clone(&armed);
        let hook_dispatches = Rc::clone(&dispatches);
        let hook_cancelled = Arc::clone(&cancelled);
        let outcome = owner.run_registration_one_shot(|operation| {
            let namespace = operation.namespace_bytes().map_err(|error| error.code().to_owned())?;
            let log = operation.catalog_bytes().map_err(|error| error.code().to_owned())?;
            let root = operation.root().to_owned();
            let source = crate::plaintext_direct_store::DirectStore::from_existing_bytes_checked(
                &root, &namespace, &log, || operation.migration_checkpoint().map_err(|error| error.code().to_owned()))?;
            let registration = operation.registration_bytes()?;
            let selected = operation.selected_binding();
            let protector = operation.credential_call(|deadline, cancellation|
                RevisionProtector::for_existing_credential(namespace_id, selected, deadline, cancellation))
                .map_err(|error| error.code().to_owned())?;
            let inventory_owner = operation.migration_inventory_owner().map_err(|error| error.code().to_owned())?;
            operation.chain_migration_phase_hook_for_test(Box::new(move |phase| {
                if hook_armed.get() && phase == "migration-held-observer-dispatch" {
                    let count = hook_dispatches.get() + 1;
                    hook_dispatches.set(count);
                    if count == 2 { hook_cancelled.store(true, Ordering::Release); }
                }
            }));
            {
                let context = MigrationContext::new(operation);
                let input = NativeMigrationInput { namespace: &namespace, log: &log, protector: &protector,
                    context: &context, inventory_owner, registration: registration.as_deref() };
                let source = MigrationSource::new(&source, deadline, Some(&input));
                let mut scope = ParentReadScope::new();
                let index = scope.resolve_directory(&source, &root.join("control"))?.map_err(|_| changed())?;
                armed.set(true);
                let refusal = scope.validate(&source, index);
                armed.set(false);
                assert_eq!(refusal, Err(OwnerError::OwnerCancelledBeforeMutation.code().to_owned()));
                assert_eq!(dispatches.get(), 2, "cancel between the first actual ABI and second dispatch");
                cancelled.store(false, Ordering::Release);
                assert_eq!(scope.validate(&source, index), refusal);
                assert_eq!(source.poll(), refusal);
                assert_eq!(dispatches.get(), 2, "clearing the same Arc cannot resume work");
            }
            assert!(!operation.output_attempted());
            Ok(())
        });
        assert_eq!(outcome, Err(OwnerError::OwnerCancelledBeforeMutation.code().to_owned()));
        assert_eq!(evidence(), before);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        assert_eq!(fs::read_dir(root.join("revisions")).unwrap().count(), 0);
        assert!(!root.join("control/source-roots.tmp").exists());
        assert!(!cancelled.load(Ordering::Acquire));
        assert!(credentials.cleanup_captured(), "exact fixture credential cleanup failed; artifacts retained");
        let current = fs::canonicalize(&base).unwrap();
        assert_eq!(current, base);
        assert_eq!(current.parent(), Some(temp.as_path()));
        assert!(current.file_name().unwrap().to_str().unwrap().starts_with("search-held-chain-cancel-"));
        fs::remove_dir_all(current).unwrap();
    }
}
