# ACK and P00 proposal publication

**Verdict: acceptable for nonclaimable proposal publication.** Independent Luna reviews bind ACK
source `74ce16e897b8c37c5fc54b1729d09d64632d71cf` and final P00 source
`0afdd35afb8afcd834e8add28b6bdcfd7dc5656f`. Their six and two Git blobs respectively are preserved
exactly in integrated commit `a9c1134726e7a63aa6640164566ed0b73fc0ece1`.
No schema adoption, profile qualification, implementation authority or issued record follows.

The ACK proposal preserves the existing pre-acknowledgement `LEASED -> SUPERSEDED` transition for
the exact integration owner, with both previous-event references absent. It adds no pre-ACK revoked
genesis, retains actor/reference/reason and terminal-chain guards, and rejects mixed v1/v2 chains.
Six obligation preimages, the pinned type-registry digest and distinct operation/input/domain versions
remain explicit. Its historical 52-type basis and future cumulative 53-type target are distinguished.
Required-evidence cardinality, ticket byte bounds, revocation receipt closure, accepted v2 operation
inputs/recovery and actual trust/profile state still block activation.

The P00 proposal keeps policy mutation ownership unconfirmed and portfolio mapping/derivation
blocked. Typed record codec closure, bounds, peer roles, disclosure and TTL decisions and three-family
coverage ownership remain unresolved. It does not authorize changes to product source. All 27 final
source hashes and tagged OIDs correspond to raw Git blobs at its exact base. Independent review found
one erroneous `crates/search-contracts/src/lib.rs` hash in `ef9c204`; the final one-line correction was
reviewed separately. Historical checkout-byte hashes have a separate scope, and their original capture
commit is explicitly unrecorded. The full-master hash and bounded Part I semantic digest use distinct
bases; neither is substituted for the other.

ACK's single final parser capture exited zero on its five exact committed TOML inputs. P00's exact
blob parser capture exited zero at `ef9c204d17c2742037ac78beb6dd123c605b13b6`; later provenance labels
and the one source-hash correction were reviewed by diff without another parser invocation. That
capture does not claim to parse the final P00 bytes. The earlier P00 CRLF worktree capture is retained
separately with its original input. Empty stdout/stderr are retained alongside metadata. ACK's manifest
records the executable but no runtime version; P00's capture records Taplo 0.10.0.

The [copy manifest](evidence/ack-p00-proposals/copy-manifest.json) preserves 21 original input, metadata
and raw-output files with byte counts and SHA-256. No parser, test or build was rerun for publication.

Readiness review separately established that low-level Git object storage and CNG signing primitives
can be implemented before keys or qualification exist, behind non-authoritative APIs. The first
[Git adapter component](../../handoff/changes/2026-10-03-local-git-artifact-adapter-stage1.md) is now
explicitly frozen for native implementation. General materialization still enforces at most 16 context
sources; the exact manifest-closed P00 `search-contracts` exception alone permits 24. A later publisher
must preserve its expected parent tree and add only absent output paths before atomic publication.
