# search-candidate-validator — evidence lookup, not a proof owner

Resolve a ranked hit through the pinned source/evidence table to actual retained bytes or a supported prepared
representation. Source identity, coordinate domain and failure scope come from those real owners, not echoed
Qdrant payloads or caller-created verification fields.

[FUNCTIONS.md](FUNCTIONS.md) contains the current consumer, retired API notes and resolution rule, including
mapped-text coordinates and request cancellation versus candidate-local gaps. [Windowing](../search-continuation/FUNCTIONS.md)
owns page progress; projection only formats the resulting evidence in its final order.

The former validator crate/manifest/source and exclusive fixtures are removed. Current
[verified_query.rs](../../../bins/eliot-searchd/src/verified_query.rs) resolves pinned evidence and verifies actual
retained Source/Prepared bytes and coordinates. This directory retains the policy; it is not a wrapper or a
pending implementation of that removed validator. See [AGENTS.md](AGENTS.md).
