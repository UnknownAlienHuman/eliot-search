# Instance settings disposition

The current daemon registry exposes only `instance.mode` and `instance.data_root`; the older lock timeout,
shutdown timeout and abandoned-owner policy fields are not registered file settings.

Target:

```toml
[instance]
data_root = "..."
```

Keep a mode only when a second real composition exists. The path remains `PathBuf`, is captured once and is
validated by the actual root owner. Root/process settings require restart. There is no section digest,
`DRAIN_AND_RESTART` action receipt or owner-lifecycle configuration protocol.

A current-format root uses one manifest-last initialize/open path and one process-lifetime guard. Unsupported
old roots fail before writes and remain untouched.
