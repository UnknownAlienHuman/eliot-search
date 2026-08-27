# eliot-search-doc-worker

**Optional.** Isolated worker for document materialization such as PDF, Office, OCR and archives.

Provider-neutral: no materializer implementation is pre-selected. Admission requires an ADR plus a
qualification suite covering deployment, no-execute behavior, coordinate and loss maps, resource
limits, malformed-input isolation and removal.
