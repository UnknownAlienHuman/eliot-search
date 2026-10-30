# Local product scope

**Selected product: one OS account, one machine, plain local files.** This decision replaces earlier optional
application-encryption and restricted-client plans. It is product scope, not a claim that legacy code is gone.

## Build this

| Area | Selected behavior |
|---|---|
| Storage | One unencrypted immutable-object format; ordinary private per-user application directory. No storage-mode selector. |
| CLI and daemon | One owned child connection over stdio. No listening service, login, pairing or bearer-token layer. |
| Search scope | The user's explicit/default source selection inside a pinned view. It is not a tenant or permission system. |
| Qdrant | Same-host loopback endpoint, not a public/LAN service. No remote search/embedding service in this product. |
| Qdrant credential | Only pass an existing API key when that local server requires it. Keep it private to the bridge; do not create a vault, lease or rotation system. |
| Privacy | Retained text, prepared text, index and backups are plaintext. The application makes no encryption-at-rest promise. |

The OS account and the operator's application directory are trusted. Protection from an administrator or a
malicious process already running as that account is not a goal. Do not implement a private filesystem sandbox,
ancestor-lock graph or cryptographic protocol to make that promise. Ordinary directory permissions still matter:
do not put application state in a shared writable directory or expose the backend on all interfaces. A loopback
client URL alone does not prove that the server has no other listener; server binding/firewall remains an explicit
deployment setting, not a new endpoint-attestation service.

## Keep these protections

Source files, repository configuration and returned text remain untrusted input. Keep no-execute acquisition,
selected-root containment, safe terminal rendering, finite allocations, deadlines and cancellation. Do not run
indexed scripts, repository hooks or implicit network fetches.

Keep one real process lock, no-clobber file publication, content hashes/length checks, guarded source updates,
coherent published views and conservative deletion. They prevent corruption, stale writes and accidental data
loss; they are not encryption. A partial result is not complete evidence.

## Do not build

No application-level encryption, DEKs, DPAPI storage profile, key selection/rotation, re-encryption, secret
catalogue, per-object key leases, secure-erasure proof, grant/MAC/pairing protocol, restricted-client revocation,
remote-user authentication or multi-tenant policy. These are outside the selected product, not mandatory later
stages or placeholders waiting for implementation. A future explicit product request must change this scope first.

Do not add configuration fields, DTO variants, traits, migrations or tests for these absent features.
Keep one current object format/version and its real integrity checks, not an extensible security-profile framework.

## Remove legacy security wiring

| Existing area | Cutover with its real consumer |
|---|---|
| CLI/daemon pairing and Windows credentials | Connect the same-account child client/dispatcher; remove old constructors, MAC/grant fields and credential dependencies together. |
| Root/config DEK selection | Initialize/open plain roots directly; remove key/profile settings and receipt-driven activation. |
| Revision crypto and OS-secret crates | After caller/feature/target inspection, remove unused public source, manifests and exclusive fixtures. Do not remove shared OS file/lock operations. |
| Encrypted legacy roots | Refuse ordinary writes and leave bytes and keys intact. Do not silently relabel, decrypt, migrate or delete them. An actual user-data export request is separate work. |

Do not touch unpublished #598 branches/worktrees or import that work as a prerequisite. Preserve it as external
work, not a selected integration. Removing crypto code must not delete user keys, originals or sole retained copies.
[The working guide](../implementation/WORKING_GUIDE.md) defines the actual caller search and minimal checks.

## Change this decision only here

Architecture and owner recipes link here. When scope changes, update affected owners, entrypoints and active
issue bodies together. Historical 8.4 documents, old issue comments and existing code cannot re-enable retired
features. Track implemented behavior in the registry; a documentation change alone removes no dependency.
