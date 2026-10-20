# Query concurrency settings disposition

`[scheduler]` is not registered by the current daemon. The proposed three-class queue/concurrency/CPU/memory
platform is not a supported settings API.

The direct query path may use one small process-local semaphore and finite queue/admission limit. Each request
retains one absolute deadline, cancellation and remaining backend/source/output counters. Add a configurable
limit only when the actual handler consumes it and saturation has a concrete typed result.

Do not build a public scheduler, tickets, terminal map, resource monitor or generic live-reload receipt.
