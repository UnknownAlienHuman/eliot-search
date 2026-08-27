# search-eliot-adapter

**Cell C30 — ELIOT Adapter.**

Provider-protocol translation between ELIOT Memory OS and the Search daemon.

- **Owns:** binding and session handling; envelope framing; grant validation; capability descriptor
  projection.
- **Must not own:** index client types, ELIOT memory dispositions, task state.

Search returns candidates, coverage, freshness, provider assurance and reason codes. It never returns
an ELIOT admission disposition and never receives canonical database credentials.
