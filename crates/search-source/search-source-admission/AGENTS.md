# Working on search-source-admission

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [README](README.md) and
[source map](FUNCTIONS.md).

Do not preserve the 367 KiB v1/v2 receipt/original-input/batch framework. Keep only one small pure current
policy compiler plus pre-read and post-read decision functions.

The native source owner supplies bounded facts, performs containment/no-follow/stable identity and owns the
single body read. Admission may deny before read and narrow after read; it never owns native handles, source
identity, immutable storage, registry/currentness, access grants or durable history.

Change the daemon/source consumer directly so the exact owned body passes to the object writer. Remove
`OriginalAdmissionV2`, receipt/provenance archives, canonical observation/decision identities, v4 registry
restore and batch codecs with their last callers. Do not add V3 wrappers or forwarding packages.

Persist only a useful policy revision, closed outcome/reason and final classification in current source state.
Unsupported old roots fail before writes and remain intact.

After code, run minimal Rust 1.98 compiler/Clippy for changed admission/source/daemon consumers. Verify deny
before read, same-body retention, post-read narrowing, cancellation and content-free errors through the real
ingestion path. Old byte/golden/batch fixtures are deleted with their APIs.
