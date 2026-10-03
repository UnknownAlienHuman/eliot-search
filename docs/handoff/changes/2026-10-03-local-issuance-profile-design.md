# Standalone local issuance profile design candidate — 2026-10-03

**State:** `PROPOSED_REVIEW_PENDING` / `NON_CLAIMABLE`. Design only, based on `6e57c317233e138e178db634696cbfdc8f6bf124`. No profile instance, key, credential, trust root, signature, issued ticket/lease/event, qualification result, actor assignment, launch change, or receipt was created. Qualification remains `UNAVAILABLE`.

This proposal replaces a generic “profiles are missing” blocker with a concrete native Windows/Rust candidate for local artifact publication, approval signatures, and durable receipts for the first four issuance mutations. It runs as ordinary local Rust tooling against the repository and Windows CNG; it adds no orchestration service, database, Python/Node runtime, dependency artifact, or implicit remote write. It is independent of Eliot Governor and Memory OS. Profile definitions still require integration-owner review and registry acceptance before any runtime may use them.

## Candidate identities and decision

| Surface | Proposed identity | Version/kind | State |
| --- | --- | --- | --- |
| Artifact storage | `eliot-search/local-git-sha256-artifact-store-v1` | v1 `LOCAL_IMMUTABLE_ARTIFACT_STORE` | Not provisioned or qualified |
| Approval | `eliot-search/local-windows-cng-software-ecdsa-p256-v1` | v1 `LOCAL_APPROVAL_PROFILE` | Not provisioned or qualified |
| Durable operation receipt | `control_operation_receipt_v1` | schema v1, path `swarm/control-operation-receipts/v1/<lowercase-64-hex-operation_id>.toml` | New record kind/status needs registration |
| Actor trust snapshot/head | `actor_trust_manifest_v1` / `actor_trust_head_v1` | schema v1 proposal in the approval TOML | New record kinds/statuses need registration |

The selected store is the repository’s existing Git object database plus one integration-owned control ref. Git objects hold exact immutable bytes; one commit contains an operation’s input artifact, output artifacts, primary control record, and receipt; one compare-and-swap ref update publishes that complete tree. This uses existing native repository mechanics without introducing the service/database ADR 0003 rejects. The current candidate uses the same local repository for source reads and record publication but leaves its control ref name, repository identity, Git executable identity, and exact instance to verified provisioning.

A contained local directory could also store SHA-256-named create-only files, but publishing several output files and their receipt requires a new journal, locking/recovery protocol, and cross-file crash rules. Git already supplies immutable blobs/trees and a single expected-old-ref update point, so it is the smaller review surface for these records. The tradeoff is clear: this profile serializes only one exact local repository/ref. It provides no remote or cross-clone currentness, no independent backup, and no protection from a compromised host or forced ref rewrite.

## Artifact-store contract

`artifact-store.toml` fixes an artifact ID as `sha256-` plus lowercase SHA-256 of the exact artifact bytes. Its locator is exactly:

```text
swarm/control-artifacts/v1/sha256/<first-two-hex>/<64-lowercase-hex>.bin
```

The `ImmutableArtifactRef` carries the exact store profile ref, artifact ID, byte count, and independent SHA-256. Git object IDs are full and algorithm-tagged to the pinned repository’s object format; they are not substituted for the SHA-256. Reads resolve against the exact receipt/output commit, never a branch name. Absolute paths, ambient worktree paths, filters, line-ending conversion, symlink traversal, path escape, or provider fallback are rejected. Content-addressed paths are create-only: an existing object can be reused only after byte-count and SHA-256 readback match; a mismatch is quarantined as an integrity failure. Control record paths are also create-only and are never repaired in place.

The candidate publishes with these steps:

1. Read the exact current bound control-ref tip and validate all input records, source blobs, profile/trust refs, and create-only targets against that tree/base.
2. Write and read back exact Git blob bytes with filters disabled. Build the prospective tree and commit as unreachable objects, verify every locator/digest/signature, and include all outputs and the `RECORDED` receipt together.
3. Move the bound control ref once with `git update-ref` using the exact old object ID. A lost response or error after this possible write becomes `OUTCOME_UNKNOWN` until read-only recovery resolves it.
4. Read the new tip back through the same bound ref and independently verify its parent, receipt, input, output records, signatures, and artifacts. Only that exact readback returns mutation success.

A publication attempt never pushes remotely. Accepted objects remain append-only and reachable from the bound control ref; automatic expiry, rewrite, force-update, and purge are outside this profile. Ordinary Git maintenance must retain reachable accepted objects. A rewind, missing object, changed control ref, or unreadable repository fails closed. An external context-artifact backup and purge owner remain provisioning decisions.

Budgets in `artifact-store.toml` are proposed hard caps, not inherited grants: up to 24 context sources, 4 MiB each and 96 MiB total; a 100 MiB materialized context artifact; 1 MiB canonical input; 256 KiB per control record and 64 KiB per operation receipt; at most three 64-byte signatures; at most five artifacts and 105,906,368 aggregate artifact bytes per operation; two control records and 327,680 aggregate control-record bytes; 106,234,048 total tracked blob bytes; 16 MiB working memory using streaming; 120 seconds per mutation and 30 seconds per recovery; 100,000 artifacts/receipts and 8 GiB reachable artifact bytes per profile instance. Exceeding any cap is a typed pre-publication failure; it never deletes or truncates an accepted object. The candidate context artifact cap exceeds the 96 MiB source cap to leave bounded framing/manifest space.

## Approval, trust, and local security boundary

The approval candidate uses the Windows-native `Microsoft Software Key Storage Provider`, ECDSA P-256, SHA-256, `NCryptSignHash`, and `NCryptVerifySignature`. Each key is a persisted current-user key; `NCRYPT_MACHINE_KEY_FLAG` is not used. The root-signed mapping supplies the exact key-container name under `EliotSearch-Approval-v1-` plus 1–96 ASCII letters/digits/period/underscore/hyphen; the name is a lookup label, not identity. Opening it must reproduce the mapped public-key fingerprint. Key creation does not set `NCRYPT_OVERWRITE_KEY_FLAG`; an existing key name blocks provisioning. Signing requests `NCRYPT_SILENT_FLAG` and fails if the selected provider requires UI. The profile pins this provider and algorithm and has no fallback. It sets `NCRYPT_EXPORT_POLICY_PROPERTY` to DWORD `0` and requires `NCryptGetProperty` to read back `0`; the exact provider must still prove that export requests fail before this is accepted as non-export behavior. No private key, credential, or PIN is stored in the repository or receipt.

The proposed signature bytes are a 64-byte IEEE P1363 pair (`r[32] || s[32]`). This is an explicit candidate wire format, not a fact established by the cited Microsoft API pages; qualification must prove the CNG output/import mapping and an independent verifier’s agreement. Any other encoding is a profile version change, not a silent conversion.

For a control record, `signed_payload_sha256` is SHA-256 over the exact UTF-8/LF bytes ending immediately before the `[signature]` table. The proposed signing preimage is the exact sequence below:

```text
ASCII("ELIOT-SEARCH/APPROVAL-SIGNATURE/V1") || 0x00 ||
LP16(profile_ref ASCII) ||
LP16(record_kind ASCII) ||
u16be(schema_version) ||
raw32(signed_payload_sha256) ||
LP16(actor_identity ASCII) ||
raw32(operation_id) ||
raw32(canonical_input_sha256) ||
raw32(trust_manifest_exact_file_sha256)
```

`LP16(x)` means a u16 big-endian byte length followed by exactly those bytes. The signature artifact, complete record-file digest, and operation-receipt digest are excluded. The receipt/output consumer records the exact complete file SHA-256 externally. This preserves the repository’s separate pre-signature and full-file digest rules and prevents signature self-reference. The approval-profile TOML records this preimage and labels it as pending acceptance.

The candidate also defines a bounded public trust shape so implementation does not depend on a guessed actor mapping:

- `actor_trust_manifest_v1` is a root-signed UTF-8/LF record under `swarm/control-trust/actor-manifests/<20-digit-generation>.toml`; the u64 generation is decimal 1 through u64 max, left-padded to exactly 20 ASCII digits. Its field order is `schema_version, record_kind, status, generation, root_key_id, actors, revoked_keys, signature`. Scalars use canonical TOML assignments, actor/revocation rows use compact ordered inline tables, and the final `[signature]` table uses the exact declared trust-signature fields and immutable artifact-ref order. Strings are unescaped ASCII under each registered field grammar; unsigned integers have no sign or leading zeroes.
- Actor rows have exact order `actor_identity, key_id, key_container_name, public_key_blob_hex, allowed_operation_domains, registration_evidence_ref`; rows sort by bytewise ASCII `ActorIdentity`, domains sort bytewise ASCII, one active key per identity, maximum 128 identities and 32 domains per identity. `key_id` is SHA-256 of the decoded exact `BCRYPT_ECCPUBLIC_BLOB` bytes. Registration evidence is non-secret evidence of an explicit mapping, not proof of separate people or OS accounts.
- Revocation rows have exact order `key_id, effective_generation, reason_sha256`, sort by raw key ID, and take effect at the signed snapshot generation. Maximum 4,096 rows and 1 MiB per manifest. A later revocation blocks new operations; it does not rewrite the trust snapshot attached to a historical signature.
- `actor_trust_head_v1` lives at the exact locator `swarm/control-trust/actor-trust-head-v1.toml` in the current control-ref tree. It has order `schema_version, record_kind, status, generation, manifest_path, manifest_git_blob_id, manifest_exact_record_file_sha256, root_key_id, signature`, capped at 64 KiB. The head is root-signed and binds the current manifest path, blob, and exact file digest. The read resolves it from the same exact control-ref tip used as the compare-and-swap parent. All trust changes publish through that same ref; therefore a concurrent trust change makes the issuance compare-and-swap fail. The operation input binds the head/manifest paths, blob IDs, and exact file digests but omits the containing commit from the idempotency preimage; the receipt records the expected parent commit and uses it to construct the full immutable refs.
- The manifest/head payload signatures use the separate domain strings `ELIOT-SEARCH/ACTOR-TRUST-MANIFEST/V1` and `ELIOT-SEARCH/ACTOR-TRUST-HEAD/V1`, each followed by `0x00`, `u16be(schema_version)`, `LP16(signature_profile_ref ASCII)`, `raw32(signed_payload_sha256)`, and `raw32(root_key_id)`. Their final ordered signature fields are `root_key_id, signature_profile_ref, signed_payload_sha256, signature_artifact_ref`.

The trust root public-key blob fingerprint must be supplied and verified out of band. The root key is distinct from every actor-mapped key; sha256- plus 64 lowercase hex IDs are decoded to raw32 in trust-signature preimages. A local key or self-signed manifest is not a trust bootstrap. The v1 trust design does not support root rotation; a reviewed external root-rotation contract is needed first. The profile creates no trust root or actor keys.

For the baseline workflow, the ticket supplies exact writer and reviewer `ActorIdentity` values; the trust snapshot maps each explicitly to one distinct key ID, the operation verifies those mappings and allowed domains, and a separate reviewer task performs the review. This satisfies the software candidate only under a trusted cooperative host/launcher and explicit role assignment. CNG proves that the mapped key signed the framed bytes; it does not prove a human read them, that two same-user processes are isolated, or that same-user processes cannot request one another’s available keys. Distinct names or key hashes do not establish distinct humans. Different Windows principals, a hardware token, or stronger custody can be evaluated as a separate optional profile; no such setup is a baseline prerequisite or assumed here.

## Canonical input bytes for the first four mutations

The operation ID remains exactly the operation contract’s formula, with no delimiter inserted:

```text
SHA-256(ASCII(existing_domain_separator) || canonical_input_bytes)
```

The four existing domain strings are `materialize_context_v1`, `issue_assignment_ticket_v1`, `issue_writer_lease_v1`, and `acknowledge_writer_lease_v1`. The acknowledgement operation remains disabled until the writer acknowledgement payload/event schema gap is accepted. If the operation or acknowledgement schema changes, its exact domain and operation schema change together; v1 and v2 are never aliased.

The proposed input artifact is binary, not a TOML reserialization. Its exact byte grammar is `ASCII("ELIOT-SEARCH-CONTROL-INPUT") || 0x00 || u16be(1) || common TLVs || operation-specific nested TLVs`. Every TLV is `u16be(field ordinal) || u8(type code) || u32be(value byte length) || value bytes`; ordinals are the 1-based position in the field list below, in order, with nested ordinals restarting at 1. The operation-specific container starts with `u16be(field count)` and contains its declared nested TLVs. `NESTED_TLV` values have the same u16 count and ordinary TLV framing; immutable record and artifact refs use the existing exact field order and typed values. An optional value is a single `0x00` absent marker or `0x01` followed by the raw declared inner value, without a second header. An ordered array is a u32 count followed by repeated u32 element length and exact canonical element bytes. Unknown, duplicate, missing, out-of-order, truncated, overlong, or trailing bytes reject. Text is exact canonical ASCII under the referenced type grammar; there is no quoting, Unicode normalization, whitespace cleanup, BOM, or implicit null. SHA-256 values are 32 raw bytes; integers are fixed-width unsigned big-endian; Git object IDs include one algorithm byte plus exactly 20 SHA-1 or 32 SHA-256 bytes and must match the pinned repository format.

The fixed common field order and wire types are recorded in `operation-receipt-v1.toml`: operation schema/kind, repository, immutable source base commit, actor, optional deadline budget, optional caller cancellation token, exact artifact/approval profile definitions and provisioned instance/qualification refs, exact current trust-head and manifest locators/digests, then one operation-specific nested TLV. First-four mutations require an explicit finite deadline budget of 1–120,000 milliseconds. An absent cancellation token is one explicit `OPTIONAL_ASCII` marker; when present it is stable ASCII and must be reused for that request’s retries. Profile, trust, or instance references may not be inferred from hashes or display names.

The operation-specific TLVs are:

| Mutation | Ordered values and types |
| --- | --- |
| `materialize_context` | Context-draft repository path, full Git blob ID, exact draft SHA-256, declared-order accepted handoff array (`ImmutableRecordRef`, exact record digest, accepted API digest). Draft selectors resolve against the exact base commit. |
| `issue_assignment_ticket` | Ticket-draft path/blob/digest; context-manifest immutable ref and exact digest; context artifact ref; ordered launch/registry source refs (path/blob/digest); ordered accepted prerequisite handoffs; exact writer and reviewer `ActorIdentity`. All source bytes come from the bound base commit. |
| `issue_writer_lease` | Exact ticket/context refs and file digests; exact context artifact ref; current package lease set sorted by canonical lease ID; each lease’s ordered event chain. Active-lease exclusion is recomputed from current control state. |
| `acknowledge_writer_lease` | Exact lease/ticket/context refs and file digests; context artifact digest; accepted acknowledgement schema digest; acknowledgement canonical payload digest and artifact ref; opaque worktree ref. Disabled until the typed writer acknowledgement and matching event/domain revision are accepted. |

The limits in the TOML bound input bytes to 1 MiB, top-level fields to 23, ASCII values to 4 KiB, immutable record refs to 16 KiB, artifact refs to 8 KiB, arrays to 256 elements/512 KiB, accepted handoffs to 16, ticket source refs to 64, lease refs to 256, and each event chain to 256 refs/4,096 total event refs. Operation-specific field lists are finite and typed. The canonical artifact itself is stored and verified by exact bytes; no parser-reserializer is permitted to reconstruct its identity.

The bound control-ref parent is a transaction compare-and-swap precondition recorded in the receipt, not an idempotency input field. Current trust-head/manifest content and operation-specific relevant state are in the canonical bytes; an unrelated control commit therefore need not change a request’s ID. Immediately before publication the mutator checks the exact expected current ref and performs the CAS. A lost/failed CAS is resolved by the recovery operation before another mutation attempt.

## Receipt, replay, and outcome contract

The proposed receipt path is unique by the lowercase 64-hex operation ID. Its exact ordered payload is:

```text
schema_version, record_kind, status, identity, operation, repository, base,
actor, input, profiles, publication, output_records, output_artifacts, signature
```

`status = RECORDED` means only that the immutable receipt bytes appear in their containing Git commit tree. It is not a serialized claim that the control ref moved, that readback succeeded, or that a ticket/lease became authoritative. `publication` records the bound control-ref name, exact expected old parent, and `CAS_TO_BOUND_CONTROL_REF` intent. The new containing commit ID and complete receipt file SHA-256 remain external consumer data. The operation receipt’s signature signs its exact pre-signature payload under the approval preimage above; its signature artifact is an output, not part of its own signed input.

The receipt lists the primary record first, followed by the ordered output locators (repository path, full Git blob ID, exact file SHA-256, record kind) and artifact refs. A consumer constructs each output `ImmutableRecordRef` from the repository and exact containing receipt commit plus its locator. The receipt does not include its own containing commit or complete-file digest, and no signature bytes, generated IDs, output digests, or receipt digests enter the canonical input. This removes self-hash and signature cycles.

Runtime states and recovery results are distinct:

| State/result | Required meaning |
| --- | --- |
| `NO_WRITE` | The operation stopped before creating persistent Git objects or attempting the ref update. |
| `PREPARED_UNREACHABLE` | Input/output blobs and a prospective commit may exist, but they are not published through the bound control ref. |
| `COMMITTED` | The exact-old-value ref CAS succeeded and all output, signature, and receipt bytes passed post-write readback. |
| `OUTCOME_UNKNOWN` | A timeout, cancellation, process loss, or I/O failure happened after a publication could have occurred; no success or retry is inferred. |
| `RECOVERED_SUCCEEDED` | Read-only recovery finds the same operation ID/input and verifies the exact historical receipt, outputs, signatures, and trust snapshot captured by that operation. Later revocation does not rewrite this history. |
| `SAFE_TO_RETRY` | Read-only evidence proves the bound control ref is still the exact expected parent and no receipt/target was published. A changed ref, missing ancestry, or possible rewrite without proof is not safe to retry. |
| `CONFLICT` | The operation ID is bound to different canonical bytes, or an output/record target exists with a different digest or actor/profile binding. |
| `PRESERVE_OUTCOME_UNKNOWN` | Readback is unavailable/inconsistent, objects or refs are missing, the control history was rewritten, or absence/success cannot be proved. |

The four recovery dispositions are the existing closed outcomes; recovery is read-only. A matching operation ID and byte-identical input returns its original exact committed result. Reuse of that ID with different bytes returns `CONTROL_OPERATION_CONFLICT`. There is no blind retry after a possible write. The receipt and primary record share one commit so no partial receipt/record publication exists. The receipt stores no guessed recovery disposition and is never rewritten to convert uncertainty into success.

The state transitions remain the normative ones: materialization yields a context manifest and exactly one writer-visible artifact only; a ticket does not create a lease or authorize implementation; a lease does not expire automatically or authorize implementation before exact writer acknowledgement; acknowledgement cannot execute from this candidate until its accepted signed payload/event schema is known. These profile drafts do not create any transition or issued record.

## Qualification and acceptance boundary

The existing ticket-issuance qualification packet is `DESIGNED_NOT_EXECUTED`; its probes remain `UNAVAILABLE`. The candidate qualification lists are also all `UNAVAILABLE` and `executed = false`. Required future cases include exact raw input vectors for all four operations, TLV order/bounds/rejection, digest/signature mutation vectors, current-head/revocation checks, CNG P1363 agreement with an independent verifier, provider/no-UI/no-fallback behavior, create-only byte readback, filter/symlink/path rejection, one-ref CAS races, lost-response recovery, same-input replay/conflict, and zero false success across every crash boundary. A same-user probe must document shared-key reachability; it must not claim process or human isolation. These probes were not run here, and no `PASS` was published.

Before an implementation can become authoritative, an integration owner must review these candidate definitions, register the three profiles and trust-record kinds/statuses, close the first-four record-instance and nested enum schemas, decide the writer-acknowledgement payload/event/domain revision, and bind accepted definition, instance, and qualification refs into any future issuance. Provisioning is separate: obtain and verify the root public-key fingerprint out of band; explicitly map exact ActorIdentity values and allowed operation domains to public blobs; establish ticket writer/reviewer assignments and a trusted cooperative launcher; bind the exact repository/ref/Git/CNG runtime; produce root-signed current trust records; apply retention/backup/purge ownership; then run independent qualification and record immutable evidence. No step is inferred from a display name, matching digest, OS username, or this design commit. Root-key rotation requires its own reviewed procedure.

The three canonical-checkout gap drafts read as inputs were `2026-10-03-native-issuance-profiles.md`, `2026-10-03-control-record-instance-closure.md`, and `2026-10-03-writer-acknowledgement.md`. They remain `PROPOSED_REVIEW_PENDING` proposals; none was modified or made authoritative here. The candidate TOMLs are separate from `swarm/`, `qualification/`, launch state, ticket/context/lease directories, and canonical registries.

## Source proof

The bounded repository read set for this design was `AGENTS.md`, `docs/handoff/AUTHORITY_MAP.md`, `docs/adr/0003-executable-swarm-orchestration-contract.md`, `docs/handoff/TICKET_ISSUANCE_OPERATIONS.md` §§2–8 and 10, `swarm/RECEIPT_CANONICALIZATION.md`, `swarm/schemas/types-v1.toml`, the first-four schema files (`context-manifest-v1.toml`, `assignment-ticket-v1.toml`, `writer-lease-v1.toml`, `lease-event-v1.toml`), `swarm/control-plane-operations.toml`, `swarm/control-plane-schema.toml`, `swarm/context-manifest-instance-v1.toml`, and `qualification/ticket-issuance/**`. Relevant findings: operation IDs already use domain bytes concatenated directly with canonical input; signatures bind record kind/schema/pre-signature digest/actor/immutable context; the full file digest is external; current first-four record schemas are schema-only; and the acknowledgement operation has no accepted typed payload in its v1 event schema. The candidate therefore preserves the existing ID formula and digest boundary and treats schema closure as pending.

At the worktree baseline on 2026-10-03, local tools reported Git `2.55.0.windows.3`, repository object format `sha1`, Rust `1.98.0`, Cargo `1.98.0`, and Taplo `0.10.0`. These observations do not qualify a future profile instance; the executable path/hash and host identity remain external instance inputs.

Primary technical sources checked on 2026-10-03:

- Microsoft documents that its Software KSP supports ECDSA P-256; [CNG Key Storage Providers](https://learn.microsoft.com/en-us/windows/win32/seccertenroll/cng-key-storage-providers), updated 2026-06-23.
- For ECDSA, Microsoft documents passing a pre-hash to `NCryptSignHash`; it also defines `NCRYPT_SILENT_FLAG` to fail when the KSP would need UI. [NCryptSignHash](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/nf-ncrypt-ncryptsignhash), updated 2025-07-22.
- Microsoft documents that omitting `NCRYPT_MACHINE_KEY_FLAG` scopes a persisted key to the current user and that an existing name returns `NTE_EXISTS` when overwrite is not requested. [NCryptCreatePersistedKey](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/nf-ncrypt-ncryptcreatepersistedkey), updated 2024-05-29. The official [Key Storage Property Identifiers](https://learn.microsoft.com/en-us/windows/win32/seccng/key-storage-property-identifiers) page, updated 2025-05-08, defines `NCRYPT_EXPORT_POLICY_PROPERTY` and its zero-flags value; successful readback alone still does not replace an export-denial qualification probe.
- The Git manual documents compare-and-swap ref updates against an exact old OID; [git-update-ref](https://git-scm.com/docs/git-update-ref.html) lists no changes between 2.54 and 2.56. Git’s [data model](https://git-scm.com/docs/gitdatamodel) says created objects do not change and object IDs hash object type and contents; its current manual is 2.56.0 dated 2026-09-28. The installed local Git is 2.55.0, so actual profile provisioning must pin and qualify that exact executable.

The Microsoft sources substantiate provider/API availability, not the proposed P1363 wire encoding, provider export policy, trusted actor mapping, or human/process isolation. Those remain explicit qualification and external-trust inputs.
