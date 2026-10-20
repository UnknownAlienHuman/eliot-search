# Source acquisition settings disposition

`[source_reader]` is not registered by the current daemon. Its old retries/delay/network/hooks/filters table is
not a supported TOML API.

The source path owns one stable final-handle read with finite bytes/deadline/cancellation. Network fetch, Git
hooks, textconv and filters remain disabled by the selected source implementation, not user switches. Expose a
file-size or retry value only when the real reader consumes and enforces it.

Source acquisition settings are ordinary typed fields or private constants; they need no section descriptor,
fingerprint, live-apply receipt or separate configuration package.
