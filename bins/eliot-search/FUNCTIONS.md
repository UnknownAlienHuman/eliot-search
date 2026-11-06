# CLI — one owned daemon operation

The CLI retains local help/version, native argument capture, bounded child output, daemon selection, child completion checks, and closed response validation. It now exposes the first retained-file operation through the same one-shot child path.

<a id="capture-and-dispatch"></a>
## Syntax

- eliot-search retain-file --file PATH [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]
- Existing root operations remain available.

The CLI forwards --file as the daemon's native --source-file argument. Root settings keep their existing precedence and native path handling. It does not open the root or source file itself.

## Retained result

The daemon returns one bounded JSON line only after the source object has been read back and the current source row committed. The CLI validates the operation, random namespace/source IDs, nonzero revisions, source ObjectRef version/kind/digest/length, native file-key shape, and the verified-readback marker before printing it.

Source bytes remain in the immutable object store and are available through the typed read_source_object path. The CLI does not claim a query, excerpt, index, or evidence result. See the [Source recipe](../../crates/search-source/README.md) for retention/CAS boundaries and the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) for separate binary/library native results and final gate status.
