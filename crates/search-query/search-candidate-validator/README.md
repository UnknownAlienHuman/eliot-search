# search-candidate-validator — evidence lookup, not a proof owner

Resolve a ranked hit through the pinned source/evidence table to actual retained bytes or a supported prepared
representation. Source identity, coordinate domain and failure scope come from those real owners, not echoed
Qdrant payloads or caller-created verification fields.

[FUNCTIONS.md](FUNCTIONS.md) contains the inspected API defects and the single resolution recipe, including
mapped-text coordinates and request cancellation versus candidate-local gaps. [Windowing](../search-continuation/FUNCTIONS.md)
owns page progress; projection only formats the resulting evidence in its final order.

The current shared validator still compares public readback metadata and has a raw-slice-only coordinate
contract. Updating these documents does not connect or fix its Rust implementation. Change actual callers
with the retained reader; remove the duplicate proof/permit/refill structures and the crate boundary if it
no longer carries a substantial responsibility. See [AGENTS.md](AGENTS.md).
