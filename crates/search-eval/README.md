# search-eval

**C29 — Telemetry and evaluation.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Own content-minimized observability, control-corpus evaluation and property/fault evidence without becoming a training pipeline.

## Owns

- opaque metrics and operation traces
- control-corpus harness and baseline adapters
- property/fault fixture orchestration
- latency/resource/security acceptance reports
- privacy leakage assertions

## Must not own

- raw source, unsaved buffers or query text in default logs
- hidden training or learning inputs
- treating green unit tests as product acceptance
- production crates depending on eval

- **Delivery wave:** W4 baseline / P08; acceptance W9 / P15
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
