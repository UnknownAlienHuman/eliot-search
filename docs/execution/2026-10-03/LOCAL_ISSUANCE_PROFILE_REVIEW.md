# Standalone local issuance profile proposal review

**Verdict: ACCEPTABLE_FOR_PROPOSAL_PUBLICATION**, at exact independently reviewed source
`0f691f1b3fd10755a3c8e650ddea6f9311842978`. This accepts publication of four nonclaimable design files;
it does not adopt or provision a profile, qualify an implementation, create authority or issue a ticket.

The integrated source `af568db0893f080d15cf6e5bc503040ee7205c3b` preserves those four Git blobs exactly.
Its parent `8978418` introduced
the proposal; the second commit applies the reviewed correction. No product code, canonical registry,
launch state or issued-record directory changes in this batch.

The proposal describes a Git object store with create-only publication, exact readback and one control-ref
compare-and-swap; a Windows CNG software approval profile; and a durable operation-receipt candidate.
Governor and Memory OS are not runtime dependencies. Distinct assigned actor/key identities and an
actual separate reviewer task remain necessary under the declared trusted cooperative host assumption;
the profile does not prove different people or process isolation under one Windows account.

Independent review of the initial `c209addae7dc379191ce14dc2f0b72f176e79cfa` candidate blocked publication:
its receipt could include its own signature-artifact digest in the signed output list. The correction
explicitly excludes that artifact from the receipt's pre-signature list, keeps signatures of other output
records, and places the receipt's `ImmutableSignatureRef` after the signing boundary. The containing
commit and complete-file digest remain external. The reviewer found this correction adequate for
proposal publication at the exact source above, without rerunning parsers or qualification.

Activation remains blocked by the undeclared qualified-profile-reference grammar, initial
actor-registration evidence/bootstrap procedure, accepted trust/profile schemas and operation encoding,
actual provisioning and independent executed qualification. The writer-acknowledgement v2 proposal is a
separate dependency. No self-signed local key, draft token or successful TOML parse supplies trust.

The author ran Taplo once on the three changed TOMLs, with reported exit 0. The
[capture metadata](evidence/local-issuance-profile/taplo-capture.txt) records the exact command and input
SHA-256 values; the adjacent retained stdout and stderr files are both zero bytes. The
[capture manifest](evidence/local-issuance-profile/capture-manifest.json) records their copied byte counts
and digests. Root verified copied-byte equality and the four integrated Git blobs. The original initial
parse was reported exit 0 in the tool response, but no separate raw capture was retained; it was not rerun.

A subsequent root exact-byte comparison failed: the author's parser inputs had 2, 31 and 7 CRLF
sequences respectively, while Git stored LF bytes. The
[provenance check](evidence/local-issuance-profile/parser-input-provenance.json) proves that removing
only those CR bytes reproduces each committed blob; it does not relabel the original parse as a check
of different exact bytes. Exact parser-input snapshots are retained beside the capture with `-text`
attributes; their original mixed line endings remain intact. No syntax parser was rerun and no signed
record or authority input was normalized.

These checks establish syntax and bounded proposal provenance, not cryptographic encoding, export
denial, mutation recovery, profile qualification, Search retrieval or package acceptance.
