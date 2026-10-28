# Working on settings

Read [FUNCTIONS.md](FUNCTIONS.md) and the affected daemon constructor. Add each typed setting with its real
consumer; Foundation needs only root/plain settings and input/diagnostic bounds. Do not implement a catalogue
of future fields, hot reload or a configuration platform first.

The recipe owns [capture and paths](FUNCTIONS.md#target-capture), [layer semantics](FUNCTIONS.md#layer-semantics)
and [redaction](FUNCTIONS.md#diagnostics-and-secrets). Preserve native OS paths, sparse omission and explicit
empty/false values. Invalid selected input fails before effects. Status reports actual constructed owners.

Select replacement TOML/Serde dependencies with the code cut. Move current locator/key consumers before
removing their old APIs; preserve unpublished #598 overlap without blocking plain storage. Delete registry,
fingerprint, receipt and duplicate-parser machinery with callers. Run minimal pinned check/Clippy after code.
