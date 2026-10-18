# Working on search-source-records

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [README](README.md). Trace real
producer/consumer pairs; this package's relocation history does not justify a permanent boundary.

Keep pure current-format values only while shared by actual source/control callers. Move them to the
closest owner when that removes a forwarding dependency. Do not add legacy batch, portfolio, currentness,
provenance or restore schemas to the new application.

The package performs no I/O and grants no source, native, access or query authority. Old codecs are
read-only evidence until their explicit consumer disappears; unsupported roots are not migrated or
rewritten automatically. Delete exclusive fixtures with the retired format.
