# Qdrant adapter boundary

Qdrant is the only indexed backend. Keep its SDK private to one adapter so a vendor API change does not
spread through source/query/result code. The [current architecture](../architecture/README.md) defines
the local baseline; this is an implementation boundary, not a security principal or a custom compiler.

## Keep the actual boundary

- Only the bridge depends directly on `qdrant-client`; use the pinned workspace dependency.
- Keep vendor request/response/filter types private and expose the small application API needed by real
  publication/query callers. Review that API when it changes.
- The runtime owns the actual Qdrant child, endpoint and credential. The bridge performs RPC translation.
- Check endpoint/owner binding, authenticated API access, version/build and the selected collection schema
  through real operations. Do not use a caller-supplied test report as runtime authority.
- Keep bounded upsert/query/readback, shared eligibility/IDF, exact source validation and explicit unknown
  mutation outcomes. Vendor status text is not an operator-safe structured application error by itself.

Use the existing Cargo metadata adapter to check actual dependency ownership, including renamed/target
entries. Rust visibility and review of the changed public API keep the vendor surface narrow. Compile
actual callers. There is no need for another import/module/alias/taint analyser in the product repository.

## Retire the partial source analyser

`xtask/src/qdrant_boundary/source.rs` and `source/**` contain 16 Rust files / 3,293 physical lines at
c8e38b6, including test-named and other inline/regression test code. They tokenize Rust, reconstruct
imports/aliases/module graphs and propagate vendor-type reachability. The old contract admitted that
macro-generated directives, conditional module selection, shadowing and other Rust forms still needed
compiler checks/review. Growing that partial language implementation is not the search product.

Under #214 remove this analyser and its dedicated shape fixtures/command coupling. Keep useful Cargo
metadata, pin consistency and actual adapter behavior checks. A public SDK leak should be fixed in the
adapter API; do not build another policy framework to prove it impossible across every Rust spelling.
Source token/file-size/module-name assertions and historical package-status requirements are no longer
merge prerequisites. Do not recreate them as an AST or rustdoc approval catalogue.

## Qdrant changes and evidence

When changing the vendor release, update its selected dependency/artifact, adapt private translation and
check the real calling path and schema behavior. Keep installed artifact/version information in one
existing place; only add runtime observations that actual operations use. Source-format restrictions on
how a version constant is spelled are not a release contract.

Run relevant bridge/consumer compilation and behavior tests after implementation. Execute the selected
live suite to establish query/IDF/schema/mutation/restart claims. Keep those reports in evaluation/release;
remove the `QualifiedGate` test-outcome parameter from runtime construction with its consumers, without
fabricating a passing receipt. Normal startup verifies its current service and schema, not its own future
acceptance report. Compilation or a source-pattern match does not qualify a live server.
