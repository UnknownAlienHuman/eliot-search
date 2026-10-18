# search-source-provenance

Current code reconstructs complete v2 admission policy/observation/decision/receipt inputs for legacy
registry/archive formats. It performs no I/O and grants no source or access authority.

The baseline current-format ingestion path does not need this as a separate owner. Admission evaluates the
current candidate directly; source/control state retains only the policy/profile revision, closed outcome
and bounded facts needed to explain the current source head. Equal pure inputs can be reevaluated.

Keep this package only while an explicit read-only old-format diagnostic or #598's active private work
still consumes its exact records. Do not make full original-input archives, receipt reconstitution or
canonical fragments prerequisites for retaining a local file.

After those consumers move, merge any useful small value into admission/registry and delete this package,
`ARCHIVE_V2.md`, its forwarding codecs and exclusive fixtures. Unsupported old roots remain intact rather
than being silently reinterpreted.
