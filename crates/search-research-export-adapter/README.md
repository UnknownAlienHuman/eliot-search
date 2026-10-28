# Optional Research export — design note only

No Cargo package, runtime handler or production consumer exists here. Export is not a baseline prerequisite.
The retired package contract and activation/receipt framework are preserved in
[Git history](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-research-export-adapter/FUNCTIONS.md), not implementation instructions.

Add an adapter only with a named downstream consumer and its verified current format. Map selected retained
source/prepared data into that format with bounded output, truthful revision/coordinates and explicit loss.
Use the existing evidence reader and operation deadline/cancellation. Calculate only checksums required by
that actual format; never relabel an internal digest as a different algorithm.

The adapter does not own source admission, storage, publication, ownership transfer, grants, receipt archives
or activation gates. Do not reserve a wire version, dependency list, package size or public API before a
consumer exists. Implement the smallest mapping beside its caller and change the caller in the same cut.

Follow the [delivery sequence](../../docs/implementation/README.md#delivery-sequence) and
[retained-evidence owner](../search-query/search-candidate-validator/FUNCTIONS.md). Optional export cannot
restore Architecture 8.4 waves, qualification fixtures, cutover receipts or package-only edit restrictions.
