//! Real NTFS component passes with actual materializer encoders and verifier.
//! Native source context is absent; no NativeCtx or DPAPI qualification follows.

use super::super::preparation_links::Row;
use super::*;
use search_contracts::Sha256Digest32;
use search_control_redb::migration::{
    LegacyInventoryBinding, LegacyInventoryEncoder, LegacyInventoryOwner,
};
use search_materializer::api::{
    LegacyDirectPreparationBinding, decode_legacy_direct_preparation,
    derive_legacy_direct_representation_id, derive_legacy_preparation_lookup_key,
    derive_legacy_preparation_object_id, encode_legacy_preparation_binding,
    encode_legacy_preparation_manifest, encode_legacy_preparation_reference,
    legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Existing Windows persisted key namespace. Objects explicitly select Plaintext;
// no protector is constructed and no protected bytes are decoded.
const BACKEND: &str = "windows-dpapi-credential-manager-v1";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "search-preparation-tuples-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("search-preparation-tuples-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct PlainFixture {
    store: crate::plaintext_direct_store::DirectStore,
    binding: LegacyDirectPreparationBinding,
    body: Vec<u8>,
    manifest: Vec<u8>,
    reference: [u8; LEGACY_PREPARATION_REFERENCE_BYTES],
    key: [u8; 32],
    object: [u8; 32],
    reference_path: PathBuf,
    object_path: PathBuf,
    _scratch: Scratch,
}
impl PlainFixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let data = scratch.0.join("data");
        fs::create_dir(&data).unwrap();
        let input = b"pub fn component_fixture() {}\n";
        let source_path = scratch.0.join("source.rs");
        fs::write(&source_path, input).unwrap();
        let mut store = crate::plaintext_direct_store::DirectStore::open(&data).unwrap();
        store
            .index_file(&source_path)
            .expect("existing component ingestion and retained catalog");
        assert_eq!(store.inventory_revisions().len(), 1);
        let revision = store.inventory_revisions().next().unwrap();
        let binding = LegacyDirectPreparationBinding {
            namespace: store.inventory_namespace(),
            source_id: digest(&revision.source_id).unwrap(),
            revision_id: digest(&revision.revision_id).unwrap(),
            content_digest: digest(&revision.content_digest).unwrap(),
            byte_length: revision.byte_length,
            materializer_digest: crate::direct_preparation::canonical_materializer_digest()
                .unwrap(),
            unitizer_digest: crate::direct_preparation::canonical_unitizer_digest().unwrap(),
        };
        let body = crate::direct_preparation::encode_preparation(input).unwrap();
        let frame = decode_legacy_direct_preparation(&body).unwrap();
        let representation = derive_legacy_direct_representation_id::<
            PreparationRepresentationDigest,
        >(&binding, frame.identity_marker());
        let encoded_binding = encode_legacy_preparation_binding(&binding);
        let manifest = encode_legacy_preparation_manifest(
            &encoded_binding,
            &representation,
            crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
            crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
            &body,
        )
        .unwrap();
        let key = derive_legacy_preparation_lookup_key::<PreparationRepresentationDigest>(
            &encoded_binding,
            BACKEND,
        );
        let object = derive_legacy_preparation_object_id::<PreparationRepresentationDigest>(
            &encoded_binding,
            BACKEND,
            &sha256::digest(&manifest),
        );
        let reference = encode_legacy_preparation_reference(
            &key,
            &sha256::digest(&manifest),
            manifest.len() as u64,
            LegacyPreparationProtection::Plaintext,
        )
        .unwrap();
        let base = store.inventory_root().join(LEGACY_PREPARATION_DIRECTORY);
        let reference_path = base.join(legacy_preparation_reference_relative_locator(&key));
        let object_path = base.join(legacy_preparation_object_relative_locator(
            &object,
            LegacyPreparationProtection::Plaintext,
        ));
        for path in [&reference_path, &object_path] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        fs::write(&reference_path, &reference).unwrap();
        fs::write(&object_path, &manifest).unwrap();
        Self {
            store,
            binding,
            body,
            manifest,
            reference,
            key,
            object,
            reference_path,
            object_path,
            _scratch: scratch,
        }
    }
}

fn preparation_encoder() -> LegacyInventoryEncoder {
    // Synthetic transport header only; it creates no admitted native context.
    let mut encoder = LegacyInventoryEncoder::with_profile(
        LegacyInventoryLimits::BASELINE,
        LegacyInventoryProfile::LocationV2,
    )
    .unwrap();
    encoder
        .header(LegacyInventoryBinding {
            owner: LegacyInventoryOwner {
                installation: [1; 16],
                incarnation: [2; 16],
                root: [3; 16],
                invocation: [4; 16],
                request_blake3: [5; 32],
                record_blake3: [6; 32],
                physical_blake3: [7; 32],
                epoch: 1,
                generation: 1,
                native_limits: [1; 6],
            },
            target: SourceNamespaceId::from_bytes([8; 16]),
            namespace: Sha256Digest32::from_bytes([9; 32]),
            snapshot: Sha256Digest32::from_bytes([10; 32]),
            registration: None,
        })
        .unwrap();
    for family in Family::ALL {
        if family == Family::Preparation {
            break;
        }
        encoder
            .end_family(family, Status::AbsentProven, true)
            .unwrap();
    }
    encoder
}

fn expected_rows(
    fixture: &PlainFixture,
    source: &super::super::MigrationSource<'_, '_, '_>,
) -> Vec<Row> {
    let subject = |path: &Path| io::locator(source, path).unwrap();
    let row = |kind, refs, counts| Row {
        status: Status::Present,
        kind,
        refs,
        counts,
    };
    let directory = |path: &Path| row(36, [subject(path), ZERO, ZERO], [0; 3]);
    let manifest_digest = sha256::digest(&fixture.manifest);
    let reference_digest = sha256::digest(&fixture.reference);
    let base = source.inventory_root().join(LEGACY_PREPARATION_DIRECTORY);
    vec![
        row(
            37,
            [
                subject(&fixture.reference_path),
                fixture.key,
                reference_digest,
            ],
            [fixture.reference.len() as u64, 0, 0],
        ),
        row(
            12,
            [fixture.key, manifest_digest, reference_digest],
            [
                fixture.manifest.len() as u64,
                u64::from(LegacyPreparationProtection::Plaintext.tag()),
                LEGACY_PREPARATION_REFERENCE_BYTES as u64,
            ],
        ),
        directory(fixture.reference_path.parent().unwrap()),
        directory(&base.join(PrepTree::References.directory_name())),
        row(
            25,
            [
                subject(&fixture.object_path),
                manifest_digest,
                sha256::digest(&encode_legacy_preparation_binding(&fixture.binding)),
            ],
            [
                fixture.manifest.len() as u64,
                crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
                crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
            ],
        ),
        row(
            26,
            [
                fixture.binding.source_id,
                fixture.binding.revision_id,
                fixture.binding.content_digest,
            ],
            [fixture.binding.byte_length, fixture.body.len() as u64, 0],
        ),
        directory(fixture.object_path.parent().unwrap()),
        directory(&base.join(PrepTree::Objects.directory_name())),
        row(
            29,
            [fixture.key, fixture.object, manifest_digest],
            [fixture.manifest.len() as u64, 0, 0],
        ),
        row(
            21,
            [crate::direct_preparation::profile_digest(), ZERO, ZERO],
            [1, 1, 0],
        ),
    ]
}

#[test]
fn plaintext_pass_keeps_complete_25_26_29_tuples_order_and_residue() {
    let fixture = PlainFixture::new();
    let source = super::super::MigrationSource::new(
        &fixture.store,
        Instant::now() + Duration::from_secs(30),
        None,
    );
    assert!(source.native().is_none());
    let root = source.inventory_root();
    let rows = expected_rows(&fixture, &source);
    let residue = [rows[2], rows[3], rows[6], rows[7]];
    let mut actual = Vec::new();
    let mut sink = |bytes: &[u8]| {
        actual.extend_from_slice(bytes);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: preparation_encoder(),
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
    };
    let mut hooks = 0;
    preparation_plaintext(&mut stream, root, BACKEND, &mut || {
        hooks += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(hooks, 1);
    let view = stream
        .preparation_links
        .as_ref()
        .expect("complete joined production view");
    assert_eq!(view.semantic().unwrap(), &[rows[8]]);
    assert_eq!(view.residue().unwrap(), residue);
    assert_eq!(stream.terminal(), Status::Present);
    assert!(stream.complete);
    let terminal = stream
        .encoder
        .end_family(Family::Preparation, Status::Present, true)
        .unwrap();
    stream.send(&terminal).unwrap();
    stream.statuses = [0; 7];
    stream.complete = true;
    preparation(&mut stream, root, Family::Residue, true).unwrap();
    assert_eq!(stream.terminal(), Status::Present);
    assert!(stream.complete);
    let terminal = stream
        .encoder
        .end_family(Family::Residue, Status::Present, true)
        .unwrap();
    stream.send(&terminal).unwrap();
    drop(stream);
    let mut expected = Vec::new();
    let mut encoder = preparation_encoder();
    for (family, rows) in [
        (Family::Preparation, rows.as_slice()),
        (Family::Residue, residue.as_slice()),
    ] {
        for row in rows {
            expected.extend(
                encoder
                    .item(
                        family,
                        row.status,
                        row.kind,
                        row.refs.map(Sha256Digest32::from_bytes),
                        row.counts,
                    )
                    .unwrap(),
            );
        }
        expected.extend(encoder.end_family(family, Status::Present, true).unwrap());
    }
    assert_eq!(
        actual, expected,
        "full production plaintext pass retains exact v2 bytes and 25/26-before-29 order"
    );
}

#[test]
fn same_size_body_change_after_preliminary_view_refuses_semantic_and_residue_completion() {
    let fixture = PlainFixture::new();
    let source = super::super::MigrationSource::new(
        &fixture.store,
        Instant::now() + Duration::from_secs(30),
        None,
    );
    let root = source.inventory_root();
    let before = io::read_detailed(
        &source,
        &fixture.object_path,
        LEGACY_PREPARATION_MAX_OBJECT_BYTES,
    )
    .unwrap()
    .unwrap()
    .proof;
    let mut mutated = fixture.manifest.clone();
    *mutated.last_mut().unwrap() ^= 1;
    let mut captured = Vec::new();
    let mut sink = |bytes: &[u8]| {
        captured.extend_from_slice(bytes);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: preparation_encoder(),
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
    };
    let mut hooks = 0;
    let result = preparation_plaintext(&mut stream, root, BACKEND, &mut || {
        hooks += 1;
        let mut writer = OpenOptions::new()
            .write(true)
            .open(&fixture.object_path)
            .unwrap();
        writer.write_all(&mutated).unwrap();
        writer.sync_all().unwrap();
        drop(writer);
        let after = io::read_detailed(
            &source,
            &fixture.object_path,
            LEGACY_PREPARATION_MAX_OBJECT_BYTES,
        )
        .unwrap()
        .unwrap()
        .proof;
        assert_eq!(
            (
                before.native.volume_serial,
                before.native.file_index,
                before.bytes
            ),
            (
                after.native.volume_serial,
                after.native.file_index,
                after.bytes
            )
        );
        assert_ne!(before.raw_digest, after.raw_digest);
        Ok(())
    });
    assert_eq!(
        hooks, 1,
        "mutation occurs exactly between preliminary and physical passes"
    );
    assert_eq!(result, Err(changed()));
    assert!(
        stream.preparation_links.is_none(),
        "unjoined semantic facts cannot be installed"
    );
    assert_eq!(
        preparation(&mut stream, root, Family::Residue, true),
        Err(changed())
    );
}

#[test]
fn plaintext_component_refuses_protected_reference_without_fake_unprotect() {
    let fixture = PlainFixture::new();
    let source = super::super::MigrationSource::new(
        &fixture.store,
        Instant::now() + Duration::from_secs(30),
        None,
    );
    let root = source.inventory_root();
    let protected =
        root.join(LEGACY_PREPARATION_DIRECTORY)
            .join(legacy_preparation_object_relative_locator(
                &fixture.object,
                LegacyPreparationProtection::Protected,
            ));
    fs::rename(&fixture.object_path, &protected).unwrap();
    let reference = encode_legacy_preparation_reference(
        &fixture.key,
        &sha256::digest(&fixture.manifest),
        fixture.manifest.len() as u64,
        LegacyPreparationProtection::Protected,
    )
    .unwrap();
    fs::write(&fixture.reference_path, reference).unwrap();
    let mut captured = Vec::new();
    let mut sink = |bytes: &[u8]| {
        captured.extend_from_slice(bytes);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: preparation_encoder(),
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
    };
    let mut hooks = 0;
    assert_eq!(
        preparation_plaintext(&mut stream, root, BACKEND, &mut || {
            hooks += 1;
            Ok(())
        }),
        Err(changed())
    );
    assert_eq!(hooks, 0, "protected bytes refuse before the physical pass");
    assert!(stream.preparation_links.is_none());
    drop(stream);
    assert!(captured.is_empty());
}

#[test]
fn plaintext_manifest_projection_keeps_profile_and_representation_refusals_distinct() {
    let fixture = PlainFixture::new();
    let source = super::super::MigrationSource::new(
        &fixture.store,
        Instant::now() + Duration::from_secs(30),
        None,
    );
    let encoded_binding = encode_legacy_preparation_binding(&fixture.binding);
    let frame = decode_legacy_direct_preparation(&fixture.body).unwrap();
    let representation = derive_legacy_direct_representation_id::<PreparationRepresentationDigest>(
        &fixture.binding,
        frame.identity_marker(),
    );
    let unsupported = encode_legacy_preparation_manifest(
        &encoded_binding,
        &representation,
        crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION + 1,
        crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
        &fixture.body,
    )
    .unwrap();
    let mut foreign_representation = representation;
    foreign_representation[0] ^= 1;
    let corrupt = encode_legacy_preparation_manifest(
        &encoded_binding,
        &foreign_representation,
        crate::direct_preparation::CANONICAL_MATERIALIZER_REVISION,
        crate::direct_preparation::CANONICAL_UNITIZER_REVISION,
        &fixture.body,
    )
    .unwrap();
    for (index, (bytes, status, kind)) in [
        (unsupported, Status::Unsupported, 25),
        (corrupt, Status::Corrupt, 25),
        (
            encoded_binding[..LEGACY_PREPARATION_BINDING_BYTES - 1].to_vec(),
            Status::Corrupt,
            13,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let path = source
            .inventory_root()
            .join(format!("manifest-projection-{index}.bin"));
        fs::write(&path, &bytes).unwrap();
        let observed = io::read_detailed(&source, &path, LEGACY_PREPARATION_MAX_OBJECT_BYTES)
            .unwrap()
            .unwrap();
        let subject = io::locator(&source, &path).unwrap();
        let evidence = observed.proof.raw_digest;
        assert_eq!(evidence, sha256::digest(&bytes));
        assert_eq!(
            preparation_manifest(&source, subject, evidence, &observed.bytes).unwrap(),
            [
                Some(Row {
                    status,
                    kind,
                    refs: [subject, evidence, ZERO],
                    counts: [bytes.len() as u64, 0, 0]
                }),
                None
            ]
        );
    }
}
