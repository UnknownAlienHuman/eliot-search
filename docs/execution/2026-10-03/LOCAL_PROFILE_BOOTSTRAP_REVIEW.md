# Local profile bootstrap proposal review

**Verdict: ACCEPTABLE_FOR_PROPOSAL_PUBLICATION**, at independently reviewed source
`31756030afa50ca5574f98b32c3c833c99206d86`. The three proposal Git blobs are preserved exactly in
integrated commit `d4266f5194fdffba9791b4997c352ddd7fcd4910`. This permits publication of the design;
it does not adopt a profile, create keys or host state, qualify an implementation, or issue records.

The design specifies a distinct integration-owner action to provision, inspect and pin a root in
protected local host state. Each authoritative operation would compare an explicit caller pin with
fresh host state and the signed envelope. Rotation, final-handle containment, ACL checks, exact
readback and create-only records fail closed. Evidence, independent review and activation bind the
same profile definition, namespace, actor/key mappings, task assignments and immutable references.
The parent approval-profile draft must separately adopt those bindings before use.

The baseline assumes a trusted cooperative Windows host. Separate assigned actor/key identities and
an actual separate reviewer task provide attribution; processes sharing the Windows token can access
that account's keys and state. No process or human isolation is claimed. Governor, Memory OS, an
external signing service, a second human and hardware are not baseline prerequisites.

The author actually invoked Taplo twice. Both recorded exits are zero; the retained combined-output
files are empty. The first capture's two input snapshots match the committed TOML blobs exactly.
The second capture has the same bootstrap bytes and a temporary qualified-reference note subsequently
removed; its second input does not match the final commit. It is not evidence that different final
bytes were parsed. The command used `lint --no-auto-config --no-schema`, so these are syntax/lint
checks, not schema or semantic validation. Runtime version and separate stream captures were not
recorded in these original manifests. No parser or test was rerun for publication.

Both original capture folders are copied with byte counts and SHA-256 checks in the
[copy manifest](evidence/local-profile-bootstrap/copy-manifest.json). Their input snapshots retain
their exact bytes under the evidence directory's `-text` attributes.

The design's grammar discussion is explicitly historical at base `0f691`. Syntax was separately
accepted at `e996427`; the pure parser is integrated at `e88ab688`. Canonical type registration and
consumer conformance remain a separate change. None of those syntax decisions supplies trust.
The bootstrap records remain nonclaimable proposals with unexecuted qualification outcomes.
