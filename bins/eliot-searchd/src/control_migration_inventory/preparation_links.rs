//! Qualified link facts and one complete physical preparation denominator per pass.
use std::ffi::{OsStr, OsString};
use std::path::Path;
use search_materializer::api::{
    LegacyPreparationInventoryTree as PrepTree, LegacyPreparationReference,
    LEGACY_PREPARATION_BINDING_BYTES,
    LegacyDirectPreparationBinding, LEGACY_PREPARATION_DIRECTORY, LEGACY_PREPARATION_MAX_OBJECT_BYTES,
    decode_legacy_preparation_reference, derive_legacy_preparation_lookup_key,
    derive_legacy_preparation_object_id, encode_legacy_preparation_binding,
    legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator,
};
use super::{MigrationSource, Status, io, Work};
use super::scan::{PreparationRepresentationDigest, digest, changed};

pub(super) const MAX_LINKS: usize = 2_000_000;
const ZERO: [u8; 32] = [0; 32];

/// Encoder arguments only: no body, permission or durable authority is cached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) status: Status,
    pub(super) kind: u16,
    pub(super) refs: [[u8; 32]; 3],
    pub(super) counts: [u64; 3],
}

struct Fact {
    subject: [u8; 32],
    observation: Result<io::ReadProof, Status>,
    seen: bool,
}

pub(super) struct View {
    read_scope: io::ParentReadScope,
    references: Vec<[u8; 32]>,
    objects: Vec<([u8; 32], OsString)>,
    object_key_bytes: usize,
    reference_facts: Vec<Fact>,
    object_facts: Vec<Fact>,
    semantic: Vec<Row>,
    residue: Vec<Row>,
    // Every enumerated physical entry belongs to exactly one side. Directory
    // outcome rows are retained separately in residue, in the original order.
    physical_entries: u64,
    positive_entries: u64,
    residue_entries: u64,
    complete: bool,
}

impl View {
    const fn empty() -> Self {
        Self {
            read_scope: io::ParentReadScope::new(),
            references: Vec::new(), objects: Vec::new(), object_key_bytes: 32,
            reference_facts: Vec::new(), object_facts: Vec::new(),
            semantic: Vec::new(), residue: Vec::new(),
            physical_entries: 0, positive_entries: 0, residue_entries: 0, complete: false,
        }
    }
    #[cfg(test)]
    pub(super) fn fixture_reads(source: &MigrationSource<'_, '_, '_>,
        entries: &[(PrepTree, [u8; 32], std::path::PathBuf)],
    ) -> Result<Self, String> {
        let mut view = Self::empty();
        for (tree, id, path) in entries {
            let facts = match tree {
                PrepTree::References => &mut view.reference_facts,
                PrepTree::Objects => &mut view.object_facts,
            };
            // This fixture isolates the native/raw-byte join from the stronger
            // production parent custody, so replacement can actually occur.
            reserve_slot(source, facts)?;
            let subject = io::locator(source, path)?;
            let read = io::read_detailed(source, path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)?;
            let observation = read.as_ref().map(|read| read.proof).map_err(|status| *status);
            push(source, facts, Fact { subject, observation, seen: false })?;
            if read.is_err() { return Err(changed()); }
            match tree {
                PrepTree::References => push(source, &mut view.references, *id)?,
                PrepTree::Objects => push(source, &mut view.objects,
                    (*id, path.file_name().ok_or_else(changed)?.to_os_string()))?,
            }
        }
        io::order_metadata(source, &mut view.references, 32, Ord::cmp)?;
        io::order_metadata(source, &mut view.objects, 256, Ord::cmp)?;
        view.object_key_bytes = 256;
        order_facts(source, &mut view.reference_facts)?;
        order_facts(source, &mut view.object_facts)?;
        Ok(view)
    }
    fn reference(&self, physical: &[u8; 32]) -> bool {
        self.references.binary_search(physical).is_ok()
    }
    fn object(&self, physical: &[u8; 32], name: &OsStr) -> bool {
        self.objects.binary_search_by(|(id, candidate)|
            id.cmp(physical).then_with(|| candidate.as_os_str().cmp(name))).is_ok()
    }
    pub(super) fn observe(&mut self, source: &MigrationSource<'_, '_, '_>, tree: PrepTree,
        subject: [u8; 32], observation: Result<io::ReadProof, Status>,
    ) -> Result<(), String> {
        let facts = match tree {
            PrepTree::References => &mut self.reference_facts,
            PrepTree::Objects => &mut self.object_facts,
        };
        admit_search(source, facts.len(), std::mem::size_of::<Fact>())?;
        if let Ok(index) = facts.binary_search_by_key(&subject, |fact| fact.subject) {
            let fact = &mut facts[index];
            if fact.seen || fact.observation != observation { return Err(changed()); }
            fact.seen = true;
        }
        Ok(())
    }
    pub(super) fn take_read_scope(&mut self) -> io::ParentReadScope {
        std::mem::replace(&mut self.read_scope, io::ParentReadScope::new())
    }
    pub(super) fn restore_read_scope(&mut self, scope: io::ParentReadScope) {
        self.read_scope = scope;
    }
    pub(super) fn entry(&mut self, positive: bool) -> Result<(), String> {
        self.physical_entries = self.physical_entries.checked_add(1).ok_or_else(limit)?;
        let count = if positive { &mut self.positive_entries } else { &mut self.residue_entries };
        *count = count.checked_add(1).ok_or_else(limit)?;
        Ok(())
    }
    pub(super) fn retain_residue(&mut self, source: &MigrationSource<'_, '_, '_>, row: Row) -> Result<(), String> {
        push(source, &mut self.residue, row)
    }
    pub(super) fn finish_physical(&mut self, source: &MigrationSource<'_, '_, '_>) -> Result<(), String> {
        let count = self.reference_facts.len().checked_add(self.object_facts.len()).ok_or_else(limit)?;
        source.charge(Work::Compare(count.checked_mul(std::mem::size_of::<Fact>()).ok_or_else(limit)?))?;
        if self.reference_facts.iter().chain(&self.object_facts)
            .any(|fact| fact.observation.is_ok() && !fact.seen)
            || self.positive_entries.checked_add(self.residue_entries) != Some(self.physical_entries)
        { return Err(changed()); }
        source.checkpoint()?;
        self.complete = true;
        Ok(())
    }
    pub(super) fn semantic(&self) -> Result<&[Row], String> {
        if !self.complete { return Err(changed()); }
        Ok(&self.semantic)
    }
    pub(super) fn residue(&self) -> Result<&[Row], String> {
        if !self.complete { return Err(changed()); }
        Ok(&self.residue)
    }
}

fn reserve_slot<T>(source: &MigrationSource<'_, '_, '_>, values: &mut Vec<T>) -> Result<(), String> {
    if values.len() == MAX_LINKS { return Err(limit()); }
    if values.len() == values.capacity() {
        let capacity = values.capacity().saturating_mul(2).clamp(32, MAX_LINKS);
        let bytes = capacity.checked_mul(std::mem::size_of::<T>()).ok_or_else(limit)?;
        source.charge(Work::Allocate(bytes))?;
        source.charge(Work::Compare(values.len().checked_mul(std::mem::size_of::<T>()).ok_or_else(limit)?))?;
        values.try_reserve_exact(capacity - values.len()).map_err(|_| limit())?;
    }
    Ok(())
}

fn push<T>(source: &MigrationSource<'_, '_, '_>, values: &mut Vec<T>, value: T) -> Result<(), String> {
    reserve_slot(source, values)?;
    source.charge(Work::Compare(std::mem::size_of::<T>()))?;
    values.push(value);
    Ok(())
}

fn limit() -> String { "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned() }

fn read_fact(source: &MigrationSource<'_, '_, '_>, path: &Path, facts: &mut Vec<Fact>,
    scope: &mut io::ParentReadScope,
)
    -> Result<Result<io::DetailedRead, Status>, String> {
    // Admit actual storage before any body open/read/hash. No provisional
    // Present or synthetic native observation enters the exact join.
    reserve_slot(source, facts)?;
    let subject = io::locator(source, path)?;
    let read = io::read_detailed_scoped(source, path, LEGACY_PREPARATION_MAX_OBJECT_BYTES, scope)?;
    let observation = read.as_ref().map(|read| read.proof).map_err(|status| *status);
    push(source, facts, Fact { subject, observation, seen: false })?;
    Ok(read)
}

fn order_facts(source: &MigrationSource<'_, '_, '_>, facts: &mut Vec<Fact>) -> Result<(), String> {
    io::order_metadata(source, facts, std::mem::size_of::<Fact>(), |left, right| left.subject.cmp(&right.subject))?;
    source.charge(Work::Compare(facts.len().checked_mul(std::mem::size_of::<Fact>()).ok_or_else(limit)?))?;
    let mut conflict = false;
    facts.dedup_by(|right, left| {
        if right.subject != left.subject { return false; }
        conflict |= right.observation != left.observation;
        true
    });
    if conflict { return Err(changed()); }
    Ok(())
}

pub(super) type SemanticRead<'a> = &'a mut dyn FnMut(&[u8; LEGACY_PREPARATION_BINDING_BYTES],
    [u8; 32], [u8; 32], LegacyPreparationReference, io::DetailedRead) -> Result<Row, String>;

pub(super) fn observe_links(source: &MigrationSource<'_, '_, '_>, root: &Path, backend: &str,
    semantic_read: SemanticRead<'_>,
) -> Result<View, String> {
    let locator_bytes = root.as_os_str().len().checked_add(2048)
        .and_then(|bytes| bytes.checked_mul(2)).ok_or_else(limit)?;
    let materializer = crate::direct_preparation::canonical_materializer_digest().map_err(str::to_owned)?;
    let unitizer = crate::direct_preparation::canonical_unitizer_digest().map_err(str::to_owned)?;
    let mut view = View::empty();
    for revision in source.inventory_revisions() {
        source.charge(Work::Allocate(512))?;
        source.charge(Work::Compare(512))?;
        let binding = encode_legacy_preparation_binding(&LegacyDirectPreparationBinding {
            namespace: source.inventory_namespace(), source_id: digest(&revision.source_id)?,
            revision_id: digest(&revision.revision_id)?, content_digest: digest(&revision.content_digest)?,
            byte_length: revision.byte_length, materializer_digest: materializer, unitizer_digest: unitizer,
        });
        let key = derive_legacy_preparation_lookup_key::<PreparationRepresentationDigest>(&binding,
            backend);
        source.charge(Work::AllocateLocator(locator_bytes))?;
        let path = root.join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_reference_relative_locator(&key));
        let row = match read_fact(source, &path, &mut view.reference_facts, &mut view.read_scope)? {
            Err(status) => Row { status: missing_gap(status), kind: 28,
                refs: [key, digest(&revision.revision_id)?, ZERO], counts: [0; 3] },
            Ok(read) => match decode_legacy_preparation_reference(&read.bytes, &key) {
                Err(_) => Row { status: Status::Corrupt, kind: 28,
                    refs: [key, read.proof.raw_digest, ZERO], counts: [read.proof.bytes, 0, 0] },
                Ok(reference) => {
                    let object = derive_legacy_preparation_object_id::<PreparationRepresentationDigest>(&binding,
                        backend, reference.manifest_digest());
                    source.charge(Work::AllocateLocator(locator_bytes))?;
                    let relative = legacy_preparation_object_relative_locator(&object, reference.protection());
                    let target = root.join(LEGACY_PREPARATION_DIRECTORY).join(&relative);
                    let refs = [key, object, *reference.manifest_digest()];
                    match read_fact(source, &target, &mut view.object_facts, &mut view.read_scope)? {
                        Err(status) => Row { status: missing_gap(status), kind: 29,
                            refs, counts: [reference.manifest_bytes(), 0, 0] },
                        Ok(encoded) => {
                            let name = Path::new(&relative).file_name().ok_or_else(changed)?;
                            source.charge(Work::Allocate(name.len().saturating_mul(2)))?;
                            // Both link members require an actual qualified target read.
                            push(source, &mut view.references, key)?;
                            push(source, &mut view.objects, (object, name.to_os_string()))?;
                            semantic_read(&binding, key, object, reference, encoded)?
                        }
                    }
                }
            }
        };
        push(source, &mut view.semantic, row)?;
    }
    io::order_metadata(source, &mut view.references, 32, Ord::cmp)?;
    view.references.dedup();
    let name_bytes = view.objects.iter().map(|(_, name)| name.len().saturating_mul(2)).max().unwrap_or(0);
    io::order_metadata(source, &mut view.objects, 32 + name_bytes,
        |left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))?;
    view.objects.dedup();
    view.object_key_bytes = 32 + name_bytes;
    order_facts(source, &mut view.reference_facts)?;
    order_facts(source, &mut view.object_facts)?;
    Ok(view)
}

fn missing_gap(status: Status) -> Status {
    if status == Status::AbsentProven { Status::Gap } else { status }
}

// Membership-only component fixtures have no native protector/context. Their
// payload remains Gap; production always supplies the actual semantic verifier.
#[cfg(test)]
pub(super) fn view(source: &MigrationSource<'_, '_, '_>, root: &Path) -> Result<View, String> {
    let input = source.native().ok_or_else(changed)?;
    observe_links(source, root, input.protector.backend_name(), &mut |_, key, object, reference, _| Ok(Row {
        status: Status::Gap, kind: 29, refs: [key, object, *reference.manifest_digest()],
        counts: [reference.manifest_bytes(), 0, 0],
    }))
}

fn admit_search(source: &MigrationSource<'_, '_, '_>, count: usize, key_bytes: usize) -> Result<(), String> {
    let comparisons = if count < 2 { count } else {
        usize::try_from((count - 1).ilog2() + 2).map_err(|_| limit())?
    };
    source.charge(Work::Compare(2 * key_bytes * comparisons))
}

pub(super) fn is_linked(source: &MigrationSource<'_, '_, '_>, view: &View,
    tree: PrepTree, physical: &[u8; 32], name: &OsStr,
) -> Result<bool, String> {
    let (count, key_bytes) = match tree {
        PrepTree::References => (view.references.len(), 32),
        PrepTree::Objects => (view.objects.len(), view.object_key_bytes),
    };
    admit_search(source, count, key_bytes)?;
    Ok(match tree {
        PrepTree::References => view.reference(physical),
        PrepTree::Objects => view.object(physical, name),
    })
}
