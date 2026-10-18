# Working on search-source-provenance

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [README](README.md). Trace every remaining
consumer before changing the archive model.

This package is transitional legacy reconstitution, not a permanent source authority. Do not add new
original-input fragments, receipt formats, canonical identities or forwarding APIs for the current local
path. Preserve exact old bytes only for an explicit read-only consumer while it exists.

Current ingestion evaluates admission directly and commits a retained revision/current head. Move any
small still-useful value to its real owner and delete this package with its archive codecs and fixtures when
legacy/#598 consumers no longer require it. Do not inspect, reset or compete with #598's active private
source work.
